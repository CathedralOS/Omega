//! A selected physical arrival calls an unchanged ordinary source body.
//!
//! This join owns only ABI transport and image-entry placement. The caller
//! supplies the already selected boundary plan; provider admission, pointer
//! validity and semantic-root establishment remain with their existing owners.
//! In particular, passing a firmware pointer through this bridge grants no
//! storage authority. The body and all ordinary calls retain their original ABI.

use calling_conventions::BoundaryEntryPlan;
use diagnostics::Diagnostic;
use machine_emission::native_entry_bridge::{NativeEntryBridge, emit_native_entry_bridge};
use object_file::{ObjectPlan, ObjectSymbolHandle, RelocationKind, RelocationOrigin, SectionKind};
use semantic_vocabulary::MachineId;

use crate::ObjectArtifact;
use crate::hosted_unit_entry::{EntryShim, PreparedEntry};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalEntryBinding {
    entry: MachineId,
    plan: BoundaryEntryPlan,
    bridge: NativeEntryBridge,
}

impl PhysicalEntryBinding {
    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub const fn plan(&self) -> &BoundaryEntryPlan {
        &self.plan
    }

    /// Frame retained while the ordinary body runs, including its call return
    /// address. The body's stack demand is accounted separately.
    pub fn stack_overhead_bytes(&self) -> u64 {
        u64::from(self.bridge.frame_bytes()) + u64::from(self.bridge.call_stack_bias())
    }
}

/// Bind the selected physical calling plan to the root's ordinary native body.
/// `inbound_ordinals` joins internal scalar/structural lane order to authored
/// formal order. It is a permutation, not an inferred positional convention.
pub fn bind_physical_entry(
    artifact: &mut ObjectArtifact,
    physical: &BoundaryEntryPlan,
    inbound_ordinals: &[u32],
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
    artifact.physical_entry = Some(PhysicalEntryBinding {
        entry: artifact.entry,
        plan: physical.clone(),
        bridge,
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
        .map_err(|error| invalid(&error.to_string()))
}

pub(crate) fn prepare(artifact: &ObjectArtifact) -> Result<PreparedEntry, Diagnostic> {
    let binding = artifact
        .physical_entry
        .as_ref()
        .ok_or_else(|| invalid("physical entry binding is missing"))?;
    let mut object = artifact.object.clone();
    let mut text = artifact.text_bytes.clone();
    let (symbol, offset) = crate::hosted_unit_entry::install_entry_shim(
        &mut object,
        &mut text,
        binding.bridge.bytes(),
        "omega_physical_entry",
        &|| invalid("object has no text section"),
    )?;
    let mut relocations = artifact.relocations.clone();
    relocations.push_record(object_file::RelocationRecord {
        origin: RelocationOrigin::Materialization {
            object_symbol_handle: symbol,
        },
        section: SectionKind::Text,
        offset: offset
            .checked_add(binding.bridge.call_field_offset())
            .ok_or_else(|| invalid("physical call offset overflows"))?,
        byte_width: 4,
        symbol_handle: artifact.entry_function().symbol,
        addend: 0,
        kind: RelocationKind::X86_64Relative32,
    });
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
    symbol: ObjectSymbolHandle,
    offset: usize,
    output: &image::EmittedImageOutput,
) -> Result<(), Diagnostic> {
    let binding = artifact
        .physical_entry
        .as_ref()
        .ok_or_else(|| invalid("image has no retained physical binding"))?;
    let end = offset
        .checked_add(binding.bridge.bytes().len())
        .ok_or_else(|| invalid("physical entry extent overflows"))?;
    if binding.entry != artifact.entry
        || object.layout.entry_symbol != symbol
        || offset != artifact.text_bytes.len()
        || text.get(offset..end) != Some(binding.bridge.bytes())
        || !crate::hosted_unit_entry::unique_region(
            object,
            symbol,
            offset,
            binding.bridge.bytes().len(),
            output,
        )
    {
        return Err(invalid("physical entry lost its exact executable region"));
    }
    let expected = relocated_bridge(
        &binding.bridge,
        offset,
        artifact.entry_function().text_offset,
    )?;
    if output.final_text_bytes.get(offset..end) != Some(expected.as_slice())
        || !crate::hosted_unit_entry::pe_entry_points_to(&output.bytes, offset, &expected)
    {
        return Err(invalid(
            "PE entry or physical call differs from the selected body",
        ));
    }
    Ok(())
}

fn relocated_bridge(
    bridge: &NativeEntryBridge,
    offset: usize,
    destination: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let origin = offset
        .checked_add(bridge.call_field_offset())
        .and_then(|origin| origin.checked_add(4))
        .ok_or_else(|| invalid("physical call origin overflows"))?;
    let displacement = i64::try_from(destination)
        .ok()
        .zip(i64::try_from(origin).ok())
        .and_then(|(destination, origin)| destination.checked_sub(origin))
        .and_then(|distance| i32::try_from(distance).ok())
        .ok_or_else(|| invalid("physical call is outside rel32 range"))?;
    let mut bytes = bridge.bytes().to_vec();
    let field = bridge.call_field_offset();
    bytes
        .get_mut(field..field + 4)
        .ok_or_else(|| invalid("physical call has no relocation field"))?
        .copy_from_slice(&displacement.to_le_bytes());
    Ok(bytes)
}

fn invalid(reason: &str) -> Diagnostic {
    Diagnostic::error(format!("physical image entry: {reason}"))
}
