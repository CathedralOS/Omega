//! A selected physical arrival calls an unchanged ordinary source body.
//!
//! This join owns ABI transport, private stack residence and image placement. The caller
//! supplies the already selected boundary plan; provider admission, pointer
//! validity and semantic-root establishment remain with their existing owners.
//! In particular, passing a firmware pointer through this bridge grants no
//! storage authority. The body and all ordinary calls retain their original ABI.

use calling_conventions::BoundaryEntryPlan;
use diagnostics::Diagnostic;
use machine_emission::native_entry_bridge::{NativeEntryBridge, emit_native_entry_bridge};
use object_file::{
    ObjectPlan, ObjectSymbolHandle, RelocationKind, RelocationOrigin, SectionKind, SectionPlan,
    SymbolKind, SymbolPlan, SymbolSection,
};
use semantic_vocabulary::MachineId;

use crate::ObjectArtifact;
use crate::hosted_unit_entry::{EntryShim, PreparedEntry};

mod stack_switch;
use stack_switch::{Destination, StackSwitch};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalEntryBinding {
    entry: MachineId,
    plan: BoundaryEntryPlan,
    bridge: NativeEntryBridge,
    body_demand: crate::StackDemand,
    arrival_stack: target::TargetEntryStackGuarantee,
    stack_switch: StackSwitch,
}

/// BSS-relative disjoint residences retained by physical entry. These are not
/// semantic roots: installation must exclude them from the mapped image extent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalEntryPartitions {
    pub saved_state_offset: u64,
    pub stack_offset: u64,
    pub stack_byte_count: u64,
    pub end_offset: u64,
}

impl PhysicalEntryBinding {
    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub const fn plan(&self) -> &BoundaryEntryPlan {
        &self.plan
    }

    pub const fn arrival_stack(&self) -> &target::TargetEntryStackGuarantee {
        &self.arrival_stack
    }

    /// Frame retained while the ordinary body runs, including its call return
    /// address. The body's stack demand is accounted separately.
    pub fn stack_overhead_bytes(&self) -> u64 {
        u64::from(self.bridge.frame_bytes())
            + u64::from(self.bridge.call_stack_bias())
            + self.stack_switch.stack_overhead_bytes()
    }

    /// Account the complete emitted invocation and reserve on private backing.
    /// Even a small body uses this route: firmware stack lifetime never becomes
    /// an implicit prerequisite of a later nonreturning source handoff.
    pub fn partitions(
        &self,
        existing_bss_bytes: u64,
    ) -> Result<PhysicalEntryPartitions, Diagnostic> {
        let alignment = u64::from(self.plan.call.stack_alignment);
        let saved_state_offset = align(existing_bss_bytes, alignment)?;
        let stack_offset = align(
            saved_state_offset
                .checked_add(24)
                .ok_or_else(|| invalid("saved entry state extent overflows"))?,
            alignment,
        )?;
        let stack_byte_count = align(
            self.body_demand
                .ceiling_bytes()
                .checked_add(self.stack_overhead_bytes())
                .and_then(|bytes| bytes.checked_add(self.arrival_stack.admission_reserve_bytes()))
                .ok_or_else(|| invalid("private entry stack demand overflows"))?,
            alignment,
        )?;
        let end_offset = stack_offset
            .checked_add(stack_byte_count)
            .ok_or_else(|| invalid("private entry stack extent overflows"))?;
        Ok(PhysicalEntryPartitions {
            saved_state_offset,
            stack_offset,
            stack_byte_count,
            end_offset,
        })
    }
}

/// Bind the selected physical calling plan to the root's ordinary native body.
/// `inbound_ordinals` joins internal scalar/structural lane order to authored
/// formal order. It is a permutation, not an inferred positional convention.
pub fn bind_physical_entry(
    artifact: &mut ObjectArtifact,
    physical: &BoundaryEntryPlan,
    inbound_ordinals: &[u32],
    arrival_stack: &target::TargetEntryStackGuarantee,
) -> Result<(), Diagnostic> {
    if artifact.physical_entry.is_some() || artifact.hosted_receiver.is_some() {
        return Err(invalid("object already has an arrival binding"));
    }
    if artifact.target.object_format != target::ObjectFormat::Coff
        || artifact.target.architecture != target::Architecture::X86_64
    {
        return Err(invalid(
            "explicit physical arrival currently requires x86-64 PE",
        ));
    }
    let request = crate::private_entry_transport::native_entry_request(
        artifact,
        &physical.call,
        inbound_ordinals,
    )
    .map_err(|error| invalid(&error.to_string()))?;
    let bridge = emit_native_entry_bridge(request).map_err(|error| invalid(&error.to_string()))?;
    let stack_switch = stack_switch::emit(&bridge)?;
    let body_demand =
        crate::object_artifact::stack_demand::derive_body_stack_demand(artifact, artifact.entry)
            .map_err(|error| invalid(&format!("ordinary body stack demand: {error}")))?;
    // The prefix writes image storage, not firmware stack. The incoming
    // return address and argument area remain governed by the selected ABI.
    if !arrival_stack.admits_demand(0, u64::from(physical.call.stack_alignment)) {
        return Err(invalid(
            "physical prefix exceeds the selected arrival stack contract",
        ));
    }
    artifact.physical_entry = Some(PhysicalEntryBinding {
        entry: artifact.entry,
        plan: physical.clone(),
        bridge,
        body_demand,
        arrival_stack: arrival_stack.clone(),
        stack_switch,
    });
    Ok(())
}

/// Independent artifact replay, not another pass during normal production.
pub(crate) fn validate_binding(artifact: &ObjectArtifact) -> Result<(), Diagnostic> {
    let Some(binding) = &artifact.physical_entry else {
        return Ok(());
    };
    if binding.entry != artifact.entry || artifact.hosted_receiver.is_some() {
        return Err(invalid("selected ordinary body or arrival kind changed"));
    }
    let ordinals = binding
        .bridge
        .request()
        .parameters
        .iter()
        .map(|(_, parameter)| parameter.inbound_ordinal)
        .collect::<Vec<_>>();
    let request = crate::private_entry_transport::native_entry_request(
        artifact,
        &binding.plan.call,
        &ordinals,
    )
    .map_err(|error| invalid(&error.to_string()))?;
    if request != *binding.bridge.request() {
        return Err(invalid(
            "physical plan differs from the retained body transport",
        ));
    }
    machine_emission::native_entry_bridge::validate_native_entry_bridge(&binding.bridge)
        .map_err(|error| invalid(&error.to_string()))?;
    let demand =
        crate::object_artifact::stack_demand::derive_body_stack_demand(artifact, artifact.entry)
            .map_err(|error| invalid(&format!("ordinary body stack demand: {error}")))?;
    if demand != binding.body_demand
        || !binding
            .arrival_stack
            .admits_demand(0, u64::from(binding.plan.call.stack_alignment))
        || stack_switch::emit(&binding.bridge)? != binding.stack_switch
    {
        return Err(invalid("private stack differs from its exact invocation"));
    }
    Ok(())
}

pub(crate) fn prepare(artifact: &ObjectArtifact) -> Result<PreparedEntry, Diagnostic> {
    let binding = artifact
        .physical_entry
        .as_ref()
        .ok_or_else(|| invalid("physical entry binding is missing"))?;
    let (mut object, _partitions, saved_state, stack_top) = provision_stack(artifact, binding)?;
    let mut text = artifact.text_bytes.clone();
    let (symbol, offset) = crate::hosted_unit_entry::install_entry_shim(
        &mut object,
        &mut text,
        &binding.stack_switch.bytes,
        "omega_physical_entry",
        &|| invalid("object has no text section"),
    )?;
    let mut relocations = artifact.relocations.clone();
    for (_, fixup) in binding.stack_switch.fixups.iter() {
        let (destination, addend) = match fixup.destination {
            Destination::SavedStack => (saved_state, 0),
            Destination::SavedAccumulator => (saved_state, 8),
            Destination::SavedScratch => (saved_state, 16),
            Destination::StackTop => (stack_top, 0),
            Destination::Body => (artifact.entry_function().symbol, 0),
        };
        relocations.push_record(object_file::RelocationRecord {
            origin: RelocationOrigin::Materialization {
                object_symbol_handle: symbol,
            },
            section: SectionKind::Text,
            offset: offset
                .checked_add(fixup.field)
                .ok_or_else(|| invalid("physical call offset overflows"))?,
            byte_width: 4,
            symbol_handle: destination,
            addend,
            kind: RelocationKind::X86_64Relative32,
        });
    }
    Ok(PreparedEntry {
        object,
        text,
        relocations,
        shim: EntryShim::Physical { symbol, offset },
    })
}

pub(crate) fn validate_image(
    artifact: &ObjectArtifact,
    object: &ObjectPlan,
    text: &[u8],
    relocations: &object_file::RelocationPlan,
    symbol: ObjectSymbolHandle,
    offset: usize,
    output: &image::EmittedImageOutput,
) -> Result<(), Diagnostic> {
    let binding = artifact
        .physical_entry
        .as_ref()
        .ok_or_else(|| invalid("image has no retained physical binding"))?;
    let end = offset
        .checked_add(binding.stack_switch.bytes.len())
        .ok_or_else(|| invalid("physical entry extent overflows"))?;
    if binding.entry != artifact.entry
        || object.layout.entry_symbol != symbol
        || offset != artifact.text_bytes.len()
        || text.get(offset..end) != Some(binding.stack_switch.bytes.as_slice())
        || !crate::hosted_unit_entry::unique_region(
            object,
            symbol,
            offset,
            binding.stack_switch.bytes.len(),
            output,
        )
    {
        return Err(invalid("physical entry lost its exact executable region"));
    }
    let prepared = prepare(artifact)?;
    if prepared.object != *object || prepared.text != text || prepared.relocations != *relocations {
        return Err(invalid("physical stack residences or relocations changed"));
    }
    let original_bss = original_bss(artifact)?.map_or(0, |(_, size, _)| size as u64);
    let partitions = binding.partitions(original_bss)?;
    let bss = output.final_image_layout.bss_address;
    let alignment = u64::from(binding.plan.call.stack_alignment);
    if partitions.end_offset != output.bss_bytes as u64
        || [
            partitions.saved_state_offset,
            partitions.stack_offset,
            partitions.end_offset,
        ]
        .iter()
        .any(|offset| {
            bss.checked_add(*offset)
                .is_none_or(|address| !address.is_multiple_of(alignment))
        })
    {
        return Err(invalid(
            "private stack is not the exact aligned mapped BSS residence",
        ));
    }
    let mut expected = binding.stack_switch.bytes.clone();
    let text_address = output.final_image_layout.text_address;
    for (_, fixup) in binding.stack_switch.fixups.iter() {
        let (base, destination_offset) = match fixup.destination {
            Destination::SavedStack => (bss, partitions.saved_state_offset),
            Destination::SavedAccumulator => (bss, partitions.saved_state_offset + 8),
            Destination::SavedScratch => (bss, partitions.saved_state_offset + 16),
            Destination::StackTop => (bss, partitions.end_offset),
            Destination::Body => (text_address, artifact.entry_function().text_offset as u64),
        };
        let destination = base
            .checked_add(destination_offset)
            .ok_or_else(|| invalid("physical entry relocation destination overflows"))?;
        let origin = text_address
            .checked_add(offset as u64)
            .and_then(|address| address.checked_add(fixup.field as u64 + 4))
            .ok_or_else(|| invalid("physical entry relocation origin overflows"))?;
        let distance = i32::try_from(i128::from(destination) - i128::from(origin))
            .map_err(|_| invalid("physical entry residence is outside rel32 range"))?;
        expected[fixup.field..fixup.field + 4].copy_from_slice(&distance.to_le_bytes());
    }
    if output.final_text_bytes.get(offset..end) != Some(expected.as_slice())
        || !crate::hosted_unit_entry::pe_entry_points_to(&output.bytes, offset, &expected)
    {
        return Err(invalid(
            "PE entry or physical call differs from the selected body",
        ));
    }
    Ok(())
}

fn invalid(reason: &str) -> Diagnostic {
    Diagnostic::error(format!("physical image entry: {reason}"))
}

fn align(bytes: u64, alignment: u64) -> Result<u64, Diagnostic> {
    if !alignment.is_power_of_two() {
        return Err(invalid("private stack alignment is not a power of two"));
    }
    bytes
        .checked_add(alignment - 1)
        .map(|bytes| bytes & !(alignment - 1))
        .ok_or_else(|| invalid("private stack alignment overflows"))
}

fn original_bss(
    artifact: &ObjectArtifact,
) -> Result<Option<(arena::Handle<SectionPlan>, usize, usize)>, Diagnostic> {
    let mut sections = artifact
        .object
        .layout
        .sections
        .iter()
        .filter(|(_, section)| section.kind == SectionKind::Bss);
    let existing = sections
        .next()
        .map(|(handle, section)| (handle, section.size, section.alignment));
    if sections.next().is_some() {
        return Err(invalid("private stack requires a unique BSS section"));
    }
    Ok(existing)
}

fn provision_stack(
    artifact: &ObjectArtifact,
    binding: &PhysicalEntryBinding,
) -> Result<
    (
        ObjectPlan,
        PhysicalEntryPartitions,
        ObjectSymbolHandle,
        ObjectSymbolHandle,
    ),
    Diagnostic,
> {
    let existing = original_bss(artifact)?;
    let partitions = binding.partitions(existing.map_or(0, |(_, size, _)| size as u64))?;
    let size = usize::try_from(partitions.end_offset)
        .map_err(|_| invalid("private stack exceeds host object coordinates"))?;
    let alignment = usize::from(binding.plan.call.stack_alignment);
    let mut object = artifact.object.clone();
    if let Some((handle, _, previous_alignment)) = existing {
        let section = object.layout.sections.get_mut(handle);
        section.size = size;
        section.alignment = previous_alignment.max(alignment);
    } else {
        object.layout.sections.insert(SectionPlan {
            kind: SectionKind::Bss,
            size,
            alignment,
        });
    }
    let mut symbol_at = |name: &str, offset: u64, size| -> Result<_, Diagnostic> {
        Ok(object.layout.symbols.insert(SymbolPlan {
            name: name.into(),
            section: SymbolSection::Section(SectionKind::Bss),
            offset: usize::try_from(offset)
                .map_err(|_| invalid("entry storage offset exceeds host coordinates"))?,
            size,
            kind: SymbolKind::Object,
            import_library: String::new(),
        }))
    };
    let saved = symbol_at(
        "omega_physical_saved_state",
        partitions.saved_state_offset,
        24,
    )?;
    let stack_top = symbol_at("omega_physical_stack_top", partitions.end_offset, 0)?;
    Ok((object, partitions, saved, stack_top))
}
