/*
We define the relocation-free object plan (`RelocationFreeObjectPlan`) that
image construction links, and its five-pair 64-bit target admission rules
(`validate_relocation_free_object`). We verify dense object-local function
symbol intervals, unique semantic entry roles, canonical sorted foreign-import
symbol sequences, and in-bounds foreign and image-address patch fields before
publishing an object.
*/

use std::collections::BTreeSet;

pub mod text_section;

use crate::object_target_policy;
use machine_code::{NormalizedForeignCallResolutionKind, PlacedNormalizedForeignCallResolution};
use optimization_core::{OptimizationSelectionIdentity, TerminalRelocationFreeTextSectionIdentity};
use selected_instructions::SelectedInstructionPlanIdentity;
use semantic_vocabulary::{BoundaryMachineId, MachineId, WorkScheduleIdentity};
use target::NativeTarget;
use terminal_psi::TerminalPsiIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObjectLocalSymbolId(u64);

impl ObjectLocalSymbolId {
    pub const fn new(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub fn at(preceding: usize, index: usize) -> Option<Self> {
        u64::try_from(preceding)
            .ok()?
            .checked_add(u64::try_from(index).ok()?)?
            .checked_add(1)
            .and_then(Self::new)
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
    UnresolvedNormalizedForeignImportFieldsV1,
    UnresolvedObjectSymbolFieldsV1,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationFreeObjectNormalizedImport {
    pub symbol: ObjectLocalSymbolId,
    pub boundary: BoundaryMachineId,
    pub ordinal: u32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationFreeObjectUnresolvedForeignCall {
    pub symbol: ObjectLocalSymbolId,
    pub resolution: PlacedNormalizedForeignCallResolution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationFreeObjectPlan {
    pub source_text_section: TerminalRelocationFreeTextSectionIdentity,
    pub psi: TerminalPsiIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub selections: OptimizationSelectionIdentity,
    pub target: NativeTarget,
    pub text_section: RelocationFreeObjectTextSection,
    pub symbol_policy: RelocationFreeObjectSymbolPolicy,
    pub symbols: Vec<RelocationFreeFunctionSymbol>,
    pub semantic_entry: MachineId,
    pub semantic_entry_symbol: ObjectLocalSymbolId,
    pub normalized_imports: Vec<RelocationFreeObjectNormalizedImport>,
    pub unresolved_normalized_foreign_calls: Vec<RelocationFreeObjectUnresolvedForeignCall>,
    pub unresolved_image_addresses: Vec<machine_code::PlacedImageAddress>,
    pub relocation_record_count: u64,
    pub relocation_requirements: RelocationFreeObjectRelocationRequirements,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelocationFreeObjectError {
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
    UnresolvedForeignCallsMissing,
    RelocationCountMismatch,
    NonCanonicalForeignImportId,
    NonCanonicalForeignImportOrder,
    NonCanonicalForeignImportName,
    UnknownForeignImportSymbol,
    ForeignImportCoordinateMismatch,
    UnusedForeignImport,
    ForeignFieldOutsideTextSection,
    ForeignFieldByteWidthMismatch,
    ImageAddressesMissing,
    ImageFieldOutsideTextSection,
    LengthOverflow,
}

pub fn canonical_private_machine_symbol_name(machine: MachineId) -> String {
    format!("__omega_terminal_machine_{}", machine.get())
}

pub fn canonical_normalized_foreign_import_symbol_name(
    boundary: BoundaryMachineId,
    ordinal: u32,
) -> String {
    format!(
        "__omega_terminal_normalized_foreign_import_{}_{}",
        boundary.get(),
        ordinal
    )
}

fn foreign_field_byte_width(kind: NormalizedForeignCallResolutionKind) -> u8 {
    match kind {
        NormalizedForeignCallResolutionKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1 => 4,
        NormalizedForeignCallResolutionKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1 => 4,
    }
}

pub(crate) fn validate_relocation_free_object(
    object: &RelocationFreeObjectPlan,
) -> Result<(), RelocationFreeObjectError> {
    let Some(policy) = object_target_policy(object.target) else {
        return Err(RelocationFreeObjectError::NonCanonicalTarget);
    };
    if object.target.pointer_size != 8 || object.target.pointer_alignment != 8 {
        return Err(RelocationFreeObjectError::NonCanonicalTarget);
    }
    if object.text_section.name != policy.text_section_name {
        return Err(RelocationFreeObjectError::WrongTextSectionName);
    }
    if object.text_section.alignment != policy.text_section_alignment {
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
    match object.relocation_requirements {
        RelocationFreeObjectRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1 => {
            if object.relocation_record_count != 0
                || !object.normalized_imports.is_empty()
                || !object.unresolved_normalized_foreign_calls.is_empty()
                || !object.unresolved_image_addresses.is_empty()
            {
                return Err(RelocationFreeObjectError::RelocationsPresent);
            }
        }
        RelocationFreeObjectRelocationRequirements::UnresolvedNormalizedForeignImportFieldsV1 => {
            if object.unresolved_normalized_foreign_calls.is_empty()
                || object.normalized_imports.is_empty()
            {
                return Err(RelocationFreeObjectError::UnresolvedForeignCallsMissing);
            }
            if !object.unresolved_image_addresses.is_empty() {
                return Err(RelocationFreeObjectError::RelocationsPresent);
            }
            if object.relocation_record_count
                != u64::try_from(object.unresolved_normalized_foreign_calls.len())
                    .map_err(|_| RelocationFreeObjectError::LengthOverflow)?
            {
                return Err(RelocationFreeObjectError::RelocationCountMismatch);
            }
        }
        RelocationFreeObjectRelocationRequirements::UnresolvedObjectSymbolFieldsV1 => {
            if object.unresolved_image_addresses.is_empty() {
                return Err(RelocationFreeObjectError::ImageAddressesMissing);
            }
            if object.unresolved_normalized_foreign_calls.is_empty()
                != object.normalized_imports.is_empty()
            {
                return Err(RelocationFreeObjectError::UnresolvedForeignCallsMissing);
            }
            if object.relocation_record_count
                != u64::try_from(
                    object.unresolved_normalized_foreign_calls.len()
                        + object.unresolved_image_addresses.len(),
                )
                .map_err(|_| RelocationFreeObjectError::LengthOverflow)?
            {
                return Err(RelocationFreeObjectError::RelocationCountMismatch);
            }
            for fields in object
                .unresolved_image_addresses
                .iter()
                .map(|address| address.fields)
            {
                let last_field = match fields {
                    machine_code::ImageAddressFields::X86_64Relative32 { patch_offset } => {
                        patch_offset
                    }
                    machine_code::ImageAddressFields::Aarch64PageAddress {
                        page_patch_offset,
                        page_offset_patch_offset,
                    } => page_patch_offset.max(page_offset_patch_offset),
                };
                if last_field
                    .checked_add(4)
                    .is_none_or(|end| end > object.text_section.byte_count)
                {
                    return Err(RelocationFreeObjectError::ImageFieldOutsideTextSection);
                }
            }
        }
    }

    let mut machines = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut cursor = 0_u64;
    let mut entry_count = 0_u64;
    for (index, symbol) in object.symbols.iter().enumerate() {
        let ordinal = ObjectLocalSymbolId::at(0, index)
            .ok_or(RelocationFreeObjectError::LengthOverflow)?
            .get();
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
        0 => return Err(RelocationFreeObjectError::MissingSemanticEntry),
        1 => {}
        _ => return Err(RelocationFreeObjectError::MultipleSemanticEntries),
    }

    let mut import_symbols = std::collections::BTreeMap::new();
    let mut last_coordinate = None;
    for (index, import) in object.normalized_imports.iter().enumerate() {
        let expected = ObjectLocalSymbolId::at(object.symbols.len(), index)
            .ok_or(RelocationFreeObjectError::LengthOverflow)?;
        if import.symbol != expected {
            return Err(RelocationFreeObjectError::NonCanonicalForeignImportId);
        }
        let coordinate = (import.boundary.get(), import.ordinal);
        if let Some(last) = last_coordinate
            && coordinate <= last
        {
            return Err(RelocationFreeObjectError::NonCanonicalForeignImportOrder);
        }
        last_coordinate = Some(coordinate);
        if import.name
            != canonical_normalized_foreign_import_symbol_name(import.boundary, import.ordinal)
        {
            return Err(RelocationFreeObjectError::NonCanonicalForeignImportName);
        }
        if !names.insert(import.name.as_str()) {
            return Err(RelocationFreeObjectError::DuplicateSymbolName);
        }
        import_symbols.insert(import.symbol, coordinate);
    }
    let mut used_imports = BTreeSet::new();
    for field in &object.unresolved_normalized_foreign_calls {
        let Some(coordinate) = import_symbols.get(&field.symbol) else {
            return Err(RelocationFreeObjectError::UnknownForeignImportSymbol);
        };
        if *coordinate != (field.resolution.boundary.get(), field.resolution.ordinal) {
            return Err(RelocationFreeObjectError::ForeignImportCoordinateMismatch);
        }
        used_imports.insert(field.symbol);
        if field.resolution.field_byte_width != foreign_field_byte_width(field.resolution.kind) {
            return Err(RelocationFreeObjectError::ForeignFieldByteWidthMismatch);
        }
        let field_end = field
            .resolution
            .field_section_offset
            .checked_add(u64::from(field.resolution.field_byte_width))
            .ok_or(RelocationFreeObjectError::ForeignFieldOutsideTextSection)?;
        if field_end > object.text_section.byte_count {
            return Err(RelocationFreeObjectError::ForeignFieldOutsideTextSection);
        }
    }
    if used_imports.len() != object.normalized_imports.len() {
        return Err(RelocationFreeObjectError::UnusedForeignImport);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use machine_code::NormalizedForeignCallResolutionState;
    use selected_instructions::{SelectedBlockId, SelectedInstructionId};
    use semantic_vocabulary::OperationId;
    use terminal_psi::{SemanticFingerprint, VocabularyMarker};

    use super::{
        BoundaryMachineId, MachineId, NativeTarget, NormalizedForeignCallResolutionKind,
        ObjectLocalSymbolId, OptimizationSelectionIdentity, PlacedNormalizedForeignCallResolution,
        RelocationFreeFunctionSymbol, RelocationFreeObjectError,
        RelocationFreeObjectNormalizedImport, RelocationFreeObjectPlan,
        RelocationFreeObjectRelocationRequirements, RelocationFreeObjectSymbolLinkage,
        RelocationFreeObjectSymbolPolicy, RelocationFreeObjectSymbolRole,
        RelocationFreeObjectTextSection, RelocationFreeObjectUnresolvedForeignCall,
        SelectedInstructionPlanIdentity, TerminalPsiIdentity,
        TerminalRelocationFreeTextSectionIdentity, WorkScheduleIdentity,
        canonical_normalized_foreign_import_symbol_name, canonical_private_machine_symbol_name,
        validate_relocation_free_object,
    };

    fn plan() -> RelocationFreeObjectPlan {
        let machine = MachineId::new(7).unwrap();
        RelocationFreeObjectPlan {
            source_text_section: TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(
                b"text",
            ),
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([4; 32]),
            },
            work_schedule: WorkScheduleIdentity::new(1).unwrap(),
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
            normalized_imports: vec![],
            unresolved_normalized_foreign_calls: vec![],
            unresolved_image_addresses: Vec::new(),
            relocation_record_count: 0,
            relocation_requirements:
                RelocationFreeObjectRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
        }
    }

    fn literal_plan() -> RelocationFreeObjectPlan {
        let mut object = plan();
        object.text_section.bytes = [0x9000_0000_u32, 0x9100_0000_u32]
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect();
        object.text_section.byte_count = 8;
        object.symbols[0].byte_count = 8;
        object.unresolved_image_addresses = vec![machine_code::PlacedImageAddress {
            machine: object.semantic_entry,
            block: SelectedBlockId(0),
            instruction: SelectedInstructionId(0),
            reference: selected_instructions::SelectedImageAddressReference::ByteLiteral {
                operation: semantic_vocabulary::OperationId::new(11).unwrap(),
                place: semantic_vocabulary::PlaceId::new(2).unwrap(),
            },
            operation: semantic_vocabulary::OperationId::new(11).unwrap(),
            instruction_function_offset: 0,
            instruction_section_offset: 0,
            instruction_byte_count: 8,
            fields: machine_code::ImageAddressFields::Aarch64PageAddress {
                page_patch_offset: 0,
                page_offset_patch_offset: 4,
            },
        }];
        object.relocation_record_count = 1;
        object.relocation_requirements =
            RelocationFreeObjectRelocationRequirements::UnresolvedObjectSymbolFieldsV1;
        object
    }

    #[test]
    fn byte_literal_addresses_bind_their_requirement() {
        let object = literal_plan();
        let mut unrequired = object.clone();
        unrequired.relocation_requirements =
            RelocationFreeObjectRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1;
        unrequired.relocation_record_count = 0;
        assert_eq!(
            validate_relocation_free_object(&unrequired),
            Err(RelocationFreeObjectError::RelocationsPresent)
        );
        let mut missing = object.clone();
        missing.unresolved_image_addresses.clear();
        assert_eq!(
            validate_relocation_free_object(&missing),
            Err(RelocationFreeObjectError::ImageAddressesMissing)
        );
        let mut outside = object;
        outside.unresolved_image_addresses[0].fields =
            machine_code::ImageAddressFields::Aarch64PageAddress {
                page_patch_offset: 4,
                page_offset_patch_offset: 8,
            };
        assert_eq!(
            validate_relocation_free_object(&outside),
            Err(RelocationFreeObjectError::ImageFieldOutsideTextSection)
        );
    }

    #[test]
    fn object_validation_rejects_process_name_relocation_and_interval_corruption() {
        let mut object = plan();
        object.symbols[0].name = "main".to_owned();
        assert_eq!(
            validate_relocation_free_object(&object),
            Err(RelocationFreeObjectError::NonCanonicalSymbolName)
        );

        let mut object = plan();
        object.relocation_record_count = 1;
        assert_eq!(
            validate_relocation_free_object(&object),
            Err(RelocationFreeObjectError::RelocationsPresent)
        );

        let mut object = plan();
        object.symbols[0].section_offset = 1;
        assert_eq!(
            validate_relocation_free_object(&object),
            Err(RelocationFreeObjectError::NonDenseSymbolInterval)
        );

        let mut object = plan();
        object.target.pointer_size = 4;
        assert_eq!(
            validate_relocation_free_object(&object),
            Err(RelocationFreeObjectError::NonCanonicalTarget)
        );
    }

    #[test]
    fn two_function_object_keeps_dense_intervals_and_nonzero_entry() {
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
        assert_eq!(
            validate_relocation_free_object(&object),
            Err(RelocationFreeObjectError::NonCanonicalSymbolId)
        );
    }

    #[test]
    fn foreign_import_plan_rejects_unsorted_unused_and_mismatched_rows() {
        let boundary = BoundaryMachineId::new(3).unwrap();
        let import = |ordinal: u32, id: u64| RelocationFreeObjectNormalizedImport {
            symbol: ObjectLocalSymbolId::new(id).unwrap(),
            boundary,
            ordinal,
            name: canonical_normalized_foreign_import_symbol_name(boundary, ordinal),
        };
        let field = |symbol: u64, ordinal: u32, instruction: u32| {
            RelocationFreeObjectUnresolvedForeignCall {
                symbol: ObjectLocalSymbolId::new(symbol).unwrap(),
                resolution: PlacedNormalizedForeignCallResolution {
                    kind: NormalizedForeignCallResolutionKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1,
                    state: NormalizedForeignCallResolutionState::UnresolvedImportFieldV1,
                    caller: MachineId::new(7).unwrap(),
                    block: SelectedBlockId(0),
                    instruction: SelectedInstructionId(instruction),
                    invocation_site: OperationId::new(11).unwrap().into(),
                    boundary,
                    ordinal,
                    call_function_offset: 0,
                    call_section_offset: 0,
                    call_byte_count: 4,
                    opcode_function_offset: 0,
                    opcode_section_offset: 0,
                    field_function_offset: 0,
                    field_section_offset: 0,
                    next_instruction_function_offset: 4,
                    next_instruction_section_offset: 4,
                    field_byte_width: 4,
                    addend: 0,
                    callback_materializations: Vec::new(),
                },
            }
        };
        let mut object = plan();
        object.relocation_requirements =
            RelocationFreeObjectRelocationRequirements::UnresolvedNormalizedForeignImportFieldsV1;
        object.normalized_imports = vec![import(1, 2), import(2, 3)];
        object.unresolved_normalized_foreign_calls = vec![field(2, 1, 0), field(3, 2, 1)];
        object.relocation_record_count = 2;
        assert_eq!(validate_relocation_free_object(&object), Ok(()));

        let mut unordered = object.clone();
        unordered.normalized_imports = vec![import(2, 2), import(1, 3)];
        assert_eq!(
            validate_relocation_free_object(&unordered),
            Err(RelocationFreeObjectError::NonCanonicalForeignImportOrder)
        );

        let mut unused = object.clone();
        unused.normalized_imports.push(import(9, 4));
        assert_eq!(
            validate_relocation_free_object(&unused),
            Err(RelocationFreeObjectError::UnusedForeignImport)
        );

        let mut mismatched = object.clone();
        mismatched.unresolved_normalized_foreign_calls[1]
            .resolution
            .ordinal = 7;
        assert_eq!(
            validate_relocation_free_object(&mismatched),
            Err(RelocationFreeObjectError::ForeignImportCoordinateMismatch)
        );

        let mut unknown = object.clone();
        unknown.unresolved_normalized_foreign_calls[1].symbol =
            ObjectLocalSymbolId::new(1).unwrap();
        assert_eq!(
            validate_relocation_free_object(&unknown),
            Err(RelocationFreeObjectError::UnknownForeignImportSymbol)
        );

        let mut miscounted = object.clone();
        miscounted.relocation_record_count = 3;
        assert_eq!(
            validate_relocation_free_object(&miscounted),
            Err(RelocationFreeObjectError::RelocationCountMismatch)
        );
    }
}
