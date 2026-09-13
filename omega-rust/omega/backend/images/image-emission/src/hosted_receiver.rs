//! A hosted receiver bridge is an exact conditional image contract, not a
//! runtime grant. Its disjoint writable-image residences become usable only
//! under the selected loader/root installation occurrence. Native settlement
//! must separately establish source ZII and normal-cleanup eligibility.
//!
//! The ordinary native claim is conditional on conforming target loading and
//! fixups, process-local exclusive writable-image storage, one entry activation,
//! admitted provider behavior, and the declared completion contract. No address
//! or physical calling-plan row creates a live installed root. Shared target
//! image replay proves mapping/fixup correspondence; this module proves the
//! disjoint activation partitions and the actual bridge's pointer/control flow.

mod instructions;

use diagnostics::Diagnostic;
use program_entry_plan::{
    ProgramEntryFusedServiceEstablishment, ProgramEntryPhysicalContractPlan,
    SelectedProgramEntrySourceSignature,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedReceiverBinding {
    source: SelectedProgramEntrySourceSignature,
    physical: ProgramEntryPhysicalContractPlan,
    services: Vec<ProgramEntryFusedServiceEstablishment>,
    demand: crate::StackDemand,
    receiver_byte_count: u64,
    receiver_alignment: u64,
}

#[cfg(any(test, feature = "test-support"))]
impl crate::ObjectArtifact {
    pub fn hosted_receiver_source_mut_for_test(
        &mut self,
    ) -> Option<&mut SelectedProgramEntrySourceSignature> {
        self.hosted_receiver
            .as_mut()
            .map(|binding| &mut binding.source)
    }

    pub fn hosted_receiver_byte_count_mut_for_test(&mut self) -> Option<&mut u64> {
        self.hosted_receiver
            .as_mut()
            .map(|binding| &mut binding.receiver_byte_count)
    }

    pub fn hosted_receiver_stack_ceiling_mut_for_test(&mut self) -> Option<&mut u64> {
        self.hosted_receiver
            .as_mut()
            .map(|binding| &mut binding.demand.ceiling_bytes)
    }
}

impl HostedReceiverBinding {
    pub const fn source(&self) -> &SelectedProgramEntrySourceSignature {
        &self.source
    }
    pub const fn physical_contract(&self) -> &ProgramEntryPhysicalContractPlan {
        &self.physical
    }
    pub const fn stack_demand(&self) -> &crate::StackDemand {
        &self.demand
    }
    pub const fn receiver_byte_count(&self) -> u64 {
        self.receiver_byte_count
    }
    pub const fn receiver_alignment(&self) -> u64 {
        self.receiver_alignment
    }
}

fn invalid() -> Diagnostic {
    Diagnostic::error("macOS hosted receiver bridge lost exact contract, storage, or entry custody")
}

fn align(value: u64, alignment: u64) -> Result<u64, Diagnostic> {
    if !alignment.is_power_of_two() {
        return Err(invalid());
    }
    value
        .checked_add(alignment - 1)
        .map(|value| value & !(alignment - 1))
        .ok_or_else(invalid)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostedReceiverPartitions {
    pub saved_continuation_offset: u64,
    pub stack_offset: u64,
    pub stack_byte_count: u64,
    pub receiver_offset: u64,
    pub receiver_byte_count: u64,
    pub end_offset: u64,
}

impl HostedReceiverBinding {
    pub fn partitions(
        &self,
        existing_bss_bytes: u64,
    ) -> Result<HostedReceiverPartitions, Diagnostic> {
        let saved = align(existing_bss_bytes, 16)?;
        let stack = saved.checked_add(16).ok_or_else(invalid)?;
        let stack_bytes = align(self.demand.ceiling_bytes().max(16), 16)?;
        let stack_end = stack.checked_add(stack_bytes).ok_or_else(invalid)?;
        let receiver = align(stack_end, self.receiver_alignment.max(16))?;
        let receiver_bytes = self.receiver_byte_count.max(1);
        Ok(HostedReceiverPartitions {
            saved_continuation_offset: saved,
            stack_offset: stack,
            stack_byte_count: stack_bytes,
            receiver_offset: receiver,
            receiver_byte_count: receiver_bytes,
            end_offset: receiver.checked_add(receiver_bytes).ok_or_else(invalid)?,
        })
    }
}

pub(crate) fn prepare(
    artifact: &crate::ObjectArtifact,
) -> Result<crate::hosted_unit_entry::PreparedEntry, Diagnostic> {
    use object_file::{
        RelocationKind, RelocationOrigin, RelocationRecord, SectionKind, SectionPlan, SymbolKind,
        SymbolPlan, SymbolSection,
    };
    let binding = artifact.hosted_receiver_binding().ok_or_else(invalid)?;
    validate_binding(artifact, binding)?;
    let mut object = artifact.object.clone();
    let existing = object
        .layout
        .sections
        .iter()
        .filter(|(_, section)| section.kind == SectionKind::Bss)
        .map(|(handle, section)| (handle, section.size, section.alignment))
        .collect::<Vec<_>>();
    if existing.len() > 1 {
        return Err(invalid());
    }
    let partitions = binding.partitions(existing.first().map_or(0, |(_, size, _)| *size as u64))?;
    let end = usize::try_from(partitions.end_offset).map_err(|_| invalid())?;
    let alignment = usize::try_from(binding.receiver_alignment.max(16)).map_err(|_| invalid())?;
    if let Some((handle, _, previous_alignment)) = existing.first() {
        let section = object.layout.sections.get_mut(*handle);
        section.size = end;
        section.alignment = alignment.max(*previous_alignment);
    } else {
        object.layout.sections.insert(SectionPlan {
            kind: SectionKind::Bss,
            size: end,
            alignment,
        });
    }
    let mut symbol_at = |name: &str, offset: u64, size: u64| -> Result<_, Diagnostic> {
        Ok(object.layout.symbols.insert(SymbolPlan {
            name: name.into(),
            section: SymbolSection::Section(SectionKind::Bss),
            offset: usize::try_from(offset).map_err(|_| invalid())?,
            size: usize::try_from(size).map_err(|_| invalid())?,
            kind: SymbolKind::Object,
            import_library: String::new(),
        }))
    };
    let scratch = symbol_at(
        "omega_hosted_saved_continuation",
        partitions.saved_continuation_offset,
        16,
    )?;
    let stack_top = symbol_at(
        "omega_hosted_stack_top",
        partitions.stack_offset + partitions.stack_byte_count,
        0,
    )?;
    let receiver = symbol_at(
        "omega_hosted_receiver",
        partitions.receiver_offset,
        partitions.receiver_byte_count,
    )?;
    let offset = artifact.text_bytes.len();
    let displacement = (artifact.entry_function().text_offset as i128) - (offset as i128 + 9 * 4);
    if !offset.is_multiple_of(4)
        || displacement % 4 != 0
        || !(-134_217_728..134_217_728).contains(&displacement)
    {
        return Err(invalid());
    }
    // No access through SP occurs before switching: the original SP/LR pair
    // lives in a separate image residence, not on an assumed loader stack.
    let words = [
        0x9000_0009,
        0x9100_0129, // scratch -> x9
        0x9100_03ea, // mov x10, sp
        0xa900_792a, // stp x10, x30, [x9]
        0x9000_000a,
        0x9100_014a, // stack top -> x10
        0x9100_015f, // mov sp, x10
        0x9000_0000,
        0x9100_0000, // receiver -> x0
        0x9400_0000 | ((displacement / 4) as u32 & 0x03ff_ffff),
        0x9000_0009,
        0x9100_0129, // scratch again; callee may clobber x9
        0xa940_792a, // ldp x10, x30, [x9]
        0x9100_015f, // restore sp
        0x5280_0000, // normal Unit -> physical i32 zero
        0xd65f_03c0, // return to exact saved loader continuation
    ];
    let mut text = artifact.text_bytes.clone();
    text.extend(words.into_iter().flat_map(u32::to_le_bytes));
    let text_section = object
        .layout
        .sections
        .iter()
        .find(|(_, section)| section.kind == SectionKind::Text)
        .map(|(handle, _)| handle)
        .ok_or_else(invalid)?;
    object.layout.sections.get_mut(text_section).size = text.len();
    let symbol = object.layout.symbols.insert(SymbolPlan {
        name: "omega_macos_hosted_receiver_entry".into(),
        section: SymbolSection::Section(SectionKind::Text),
        offset,
        size: 64,
        kind: SymbolKind::Function,
        import_library: String::new(),
    });
    object.layout.entry_symbol = symbol;
    let mut relocations = artifact.relocations.clone();
    for (instruction, destination) in [(0, scratch), (4, stack_top), (7, receiver), (10, scratch)] {
        for (relative, kind) in [
            (0, RelocationKind::Aarch64Page21),
            (1, RelocationKind::Aarch64PageOffset12),
        ] {
            relocations.push_record(RelocationRecord {
                origin: RelocationOrigin::Materialization {
                    object_symbol_handle: symbol,
                },
                section: SectionKind::Text,
                offset: offset + (instruction + relative) * 4,
                byte_width: 4,
                symbol_handle: destination,
                addend: 0,
                kind,
            });
        }
    }
    Ok(crate::hosted_unit_entry::PreparedEntry {
        object,
        text,
        relocations,
        shim: crate::hosted_unit_entry::EntryShim::DarwinReceiver { symbol, offset },
    })
}

pub fn bind_macos_hosted_receiver(
    artifact: &mut crate::ObjectArtifact,
    source: &SelectedProgramEntrySourceSignature,
    physical: &ProgramEntryPhysicalContractPlan,
    services: &[ProgramEntryFusedServiceEstablishment],
    demand: &crate::StackDemand,
) -> Result<(), Diagnostic> {
    if artifact.hosted_receiver.is_some() {
        return Err(invalid());
    }
    crate::function_fragments::replay::validate(artifact)?;
    let (receiver_byte_count, receiver_alignment) = receiver_layout(artifact, source, services)?;
    let binding = HostedReceiverBinding {
        source: source.clone(),
        physical: physical.clone(),
        services: services.to_vec(),
        demand: demand.clone(),
        receiver_byte_count,
        receiver_alignment,
    };
    validate_binding(artifact, &binding)?;
    artifact.hosted_receiver = Some(binding);
    Ok(())
}

pub(crate) fn validate_binding(
    artifact: &crate::ObjectArtifact,
    binding: &HostedReceiverBinding,
) -> Result<(), Diagnostic> {
    if artifact.target != target::NativeTarget::macos_arm64()
        || !physical_contract_matches(&binding.physical)
        || binding.physical.target_slot() != binding.source.target_slot()
        || binding.source.target_slot() != target::TargetProfile::MacosArm64.program_entry_slot()
        || crate::derive_stack_demand(artifact, artifact.entry).map_err(|_| invalid())?
            != binding.demand
        || receiver_layout(artifact, &binding.source, &binding.services)?
            != (binding.receiver_byte_count, binding.receiver_alignment)
    {
        return Err(invalid());
    }
    Ok(())
}

fn physical_contract_matches(physical: &ProgramEntryPhysicalContractPlan) -> bool {
    use program_entry_plan::{MACOS_ARM64_ADDRESS_TYPE_IDENTITY, MACOS_ARM64_I32_TYPE_IDENTITY};
    let expected = program_entry_plan::exact_macos_arm64_physical_boundary_entry_plan();
    // Native settlement separately rejoins the accepted package requirement
    // identity. A package-qualified requirement is not the standalone spelling;
    // it must still carry these exact target-source bytes, role, and ABI/state.
    physical.target_slot() == target::TargetProfile::MacosArm64.program_entry_slot()
        && physical.target_package() == target::ProgramEntryPhysicalContractPackage::MacosArm64
        && physical.target_package_source_digest()
            == program_entry_plan::exact_macos_arm64_physical_contract_package_source_digest()
        && !physical.requirement_identity().is_empty()
        && physical.parameter_type_identities()
            == [
                MACOS_ARM64_I32_TYPE_IDENTITY,
                MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
                MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
                MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
            ]
        && physical.result_type_identity() == MACOS_ARM64_I32_TYPE_IDENTITY
        && physical.boundary_entry_plan() == expected.plan()
        && physical.calling_plan_report_fingerprint() == expected.contract_report_fingerprint()
        && physical.guaranteed_entry_stack().is_none()
        && physical.guaranteed_entry_stack_application().is_none()
}

/// Layout-side zero checking cannot establish authored default domains or
/// absence of nominal cleanup. Native realization must separately retain and
/// replay its checked source receipt before installing this conditional bridge.
fn receiver_layout(
    artifact: &crate::ObjectArtifact,
    source: &SelectedProgramEntrySourceSignature,
    services: &[ProgramEntryFusedServiceEstablishment],
) -> Result<(u64, u64), Diagnostic> {
    use calling_conventions::CallingPolicy;
    use terminal_psi::{
        StructuralAccess, StructuralFieldType, StructuralMultiplicity, StructuralTypeShape,
    };
    let (function, target) = crate::function_fragments::replay::entry_source(artifact)?;
    let [parameter] = function.structural_parameters.as_slice() else {
        return Err(invalid());
    };
    let [native] = target.graph.parameters.as_slice() else {
        return Err(invalid());
    };
    let receiver_identity = source
        .receiver()
        .normalized_type_identity()
        .ok_or_else(invalid)?;
    if !matches!(
        function.result,
        abstract_operations::AbstractFunctionResult::Unit
    ) || !function.parameters.is_empty()
        || !function.entry_claims.is_empty()
        || !source.visible_parameters().is_empty()
        || !parameter.is_self
        || parameter.position != 0
        || parameter.access != StructuralAccess::MutableBorrow
        || parameter.multiplicity == StructuralMultiplicity::Linear
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || function.attachment != Some(parameter.structural_type)
        || target.graph.call_plan.policy != CallingPolicy::Aapcs64
        || target.graph.call_plan.parameters.as_slice() != [native.placement.clone()]
        || target.graph.call_plan.result.is_some()
        || native.place != parameter.place
        || native.structural_type != parameter.structural_type
        || native.access != parameter.access
        || !receiver_pointer_matches(native.shape, &native.placement)
    {
        return Err(invalid());
    }
    let mut declarations = target
        .graph
        .structural_types
        .iter()
        .filter(|declaration| declaration.id == parameter.structural_type);
    let declaration = declarations.next().ok_or_else(invalid)?;
    if declarations.next().is_some() {
        return Err(invalid());
    }
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return Err(invalid());
    };
    let mut erased = 0;
    for field in fields {
        if field.relevance.is_erased()
            || matches!(field.field_type, StructuralFieldType::Erased { .. })
        {
            let StructuralFieldType::Erased { type_identity } = &field.field_type else {
                return Err(invalid());
            };
            let mut matches = services
                .iter()
                .filter(|row| row.field_identity() == field.identity);
            let row = matches.next().ok_or_else(invalid)?;
            if matches.next().is_some()
                || row.source_signature_identity() != source.identity()
                || row.target_slot() != source.target_slot()
                || row.receiver_type_identity() != receiver_identity
                || row.attachment_type_identity() != declaration.identity
                || row.carrier_type_identity() != type_identity
            {
                return Err(invalid());
            }
            erased += 1;
        } else {
            match field.field_type {
                StructuralFieldType::Scalar(
                    semantic_vocabulary::ScalarType::Boolean
                    | semantic_vocabulary::ScalarType::Integer(_),
                ) => {}
                StructuralFieldType::BoundedInteger(integer)
                    if integer.contains(semantic_vocabulary::IntegerValue::Signed(0))
                        || integer.contains(semantic_vocabulary::IntegerValue::Unsigned(0)) => {}
                StructuralFieldType::Structural(structural_type)
                    if zero_valid_record_storage(
                        &target.graph.structural_types,
                        structural_type,
                        &mut vec![parameter.structural_type],
                    ) => {}
                _ => return Err(invalid()),
            }
        }
    }
    if erased != services.len() || !native.shape.alignment.is_power_of_two() {
        return Err(invalid());
    }
    Ok((
        u64::from(native.shape.byte_size),
        u64::from(native.shape.alignment),
    ))
}

fn zero_valid_record_storage(
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    structural_type: semantic_vocabulary::StructuralTypeId,
    visiting: &mut Vec<semantic_vocabulary::StructuralTypeId>,
) -> bool {
    use semantic_vocabulary::{IntegerValue, ScalarType};
    use terminal_psi::{StructuralFieldType, StructuralTypeShape};
    if visiting.contains(&structural_type) {
        return false;
    }
    let mut matches = declarations
        .iter()
        .filter(|declaration| declaration.id == structural_type);
    let Some(declaration) = matches.next() else {
        return false;
    };
    if matches.next().is_some() {
        return false;
    }
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return terminal_semantics::scalar_array_leaf_shape(declarations.iter(), structural_type)
            .is_some_and(|(scalar, _)| {
                matches!(scalar, ScalarType::Boolean | ScalarType::Integer(_))
            });
    };
    visiting.push(structural_type);
    let valid = fields.iter().all(|field| {
        !field.relevance.is_erased()
            && match field.field_type {
                StructuralFieldType::Scalar(ScalarType::Boolean | ScalarType::Integer(_)) => true,
                StructuralFieldType::BoundedInteger(integer) => {
                    integer.contains(IntegerValue::Signed(0))
                        || integer.contains(IntegerValue::Unsigned(0))
                }
                StructuralFieldType::Structural(child) => {
                    zero_valid_record_storage(declarations, child, visiting)
                }
                // No nested service occurrence is established by this bridge.
                _ => false,
            }
    });
    visiting.pop();
    valid
}

/// The bridge supplies an address in x0, not an eight-byte receiver value.
/// Indirect placement retains the referent geometry, including empty receivers.
fn receiver_pointer_matches(
    shape: calling_conventions::ValueShape,
    placement: &calling_conventions::ValuePlacement,
) -> bool {
    use calling_conventions::{
        IndirectPointerLocation, MachineRegister, ValueClass, ValueLocation,
    };
    shape.class == ValueClass::BorrowedReference
        && shape.alignment.is_power_of_two()
        && placement.shape == shape
        && matches!(
            placement.locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(MachineRegister::Aarch64X(0)),
                copy_stack_byte_offset: None,
                byte_size,
                alignment,
            }] if *byte_size == shape.byte_size && *alignment == shape.alignment
        )
}

pub(crate) fn validate_image(
    artifact: &crate::ObjectArtifact,
    object: &object_file::ObjectPlan,
    text: &[u8],
    relocations: &object_file::RelocationPlan,
    symbol: object_file::ObjectSymbolHandle,
    offset: usize,
    output: &image::EmittedImageOutput,
) -> Result<(), Diagnostic> {
    let expected = prepare(artifact)?;
    if expected.object != *object
        || expected.text != text
        || expected.relocations != *relocations
        || !matches!(expected.shim, crate::hosted_unit_entry::EntryShim::DarwinReceiver { symbol: actual, offset: actual_offset } if actual == symbol && actual_offset == offset)
        || !crate::hosted_unit_entry::unique_region(object, symbol, offset, 64, output)
        || !crate::hosted_unit_entry::main_points_to(&output.bytes, offset)
    {
        return Err(invalid());
    }
    let binding = artifact.hosted_receiver_binding().ok_or_else(invalid)?;
    let mut original_bss = artifact
        .object
        .layout
        .sections
        .iter()
        .filter(|(_, section)| section.kind == object_file::SectionKind::Bss);
    let original_bss_bytes = original_bss
        .next()
        .map_or(0, |(_, section)| section.size as u64);
    if original_bss.next().is_some() {
        return Err(invalid());
    }
    let partitions = binding.partitions(original_bss_bytes)?;
    validate_partitions(binding, partitions, original_bss_bytes, output)?;
    // The enclosing replay has already checked the full file/VM correspondence
    // and exact dynamic fixups, including exclusion of every BSS byte. This
    // join therefore uses real mapped zero-fill, not just a section label.
    let end = offset.checked_add(64).ok_or_else(invalid)?;
    instructions::validate(
        output
            .final_text_bytes
            .get(offset..end)
            .ok_or_else(invalid)?,
        output
            .final_image_layout
            .text_address
            .checked_add(offset as u64)
            .ok_or_else(invalid)?,
        output
            .final_image_layout
            .text_address
            .checked_add(artifact.entry_function().text_offset as u64)
            .ok_or_else(invalid)?,
        output.final_image_layout.bss_address,
        partitions,
    )
}

fn validate_partitions(
    binding: &HostedReceiverBinding,
    partitions: HostedReceiverPartitions,
    original_bss_bytes: u64,
    output: &image::EmittedImageOutput,
) -> Result<(), Diagnostic> {
    let continuation_end = partitions
        .saved_continuation_offset
        .checked_add(16)
        .ok_or_else(invalid)?;
    let stack_end = partitions
        .stack_offset
        .checked_add(partitions.stack_byte_count)
        .ok_or_else(invalid)?;
    let receiver_end = partitions
        .receiver_offset
        .checked_add(partitions.receiver_byte_count)
        .ok_or_else(invalid)?;
    if partitions.saved_continuation_offset < original_bss_bytes
        || continuation_end > partitions.stack_offset
        || partitions.stack_byte_count < binding.demand.ceiling_bytes()
        || stack_end > partitions.receiver_offset
        || partitions.receiver_byte_count < binding.receiver_byte_count.max(1)
        || receiver_end != partitions.end_offset
        || partitions.end_offset != output.bss_bytes as u64
    {
        return Err(invalid());
    }
    let bss = output.final_image_layout.bss_address;
    for (offset, alignment) in [
        (partitions.saved_continuation_offset, 16),
        (partitions.stack_offset, 16),
        (stack_end, 16),
        (
            partitions.receiver_offset,
            binding.receiver_alignment.max(16),
        ),
    ] {
        if !alignment.is_power_of_two()
            || !bss
                .checked_add(offset)
                .ok_or_else(invalid)?
                .is_multiple_of(alignment)
        {
            return Err(invalid());
        }
    }
    bss.checked_add(partitions.end_offset).ok_or_else(invalid)?;
    Ok(())
}

#[cfg(test)]
mod tests;
