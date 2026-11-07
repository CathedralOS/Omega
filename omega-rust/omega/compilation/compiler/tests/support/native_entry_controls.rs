//! Check entry readback with an actual source-produced callback and image.
//!
//! Candidate bytes and plain byte inventories are observations, not accepted
//! images. Keep the retained object, ABI, body and image identity unchanged.

use std::path::Path;

pub(super) fn inspect(
    report: &compilation_products::CompileReport,
    fixture: &Path,
) -> Result<(), String> {
    let oracle = fixture.join("expected_native_entry_controls.txt");
    if !oracle.exists() {
        return Ok(());
    }
    if std::fs::read_to_string(oracle)
        .map_err(|error| error.to_string())?
        .trim()
        != "required"
    {
        return Err("native entry control oracle must say required".into());
    }
    let artifact = report
        .retained_native_artifact()
        .ok_or("native entry controls require a retained direct native image")?;
    let object = artifact.object();
    let retained = artifact.image();
    let output = retained.output();
    image_emission::validate_direct_executable_image_output(object, retained, output)
        .map_err(|error| format!("unchanged native entry readback: {error}"))?;
    let entry = object
        .private_functions()
        .iter()
        .find(|entry| entry.entry_bridge.is_some())
        .ok_or("native entry controls require an actually emitted callback adapter")?;
    let bridge = entry
        .entry_bridge
        .as_ref()
        .ok_or("selected native entry lost its adapter")?;
    let start = entry.function.text_offset;
    let end = start
        .checked_add(entry.function.byte_count)
        .ok_or("native entry control span overflows")?;
    let field = start
        .checked_add(bridge.call_field_offset())
        .ok_or("native entry control call field overflows")?;
    let original = output
        .final_text_bytes
        .get(start..end)
        .ok_or("native entry control span is outside text")?;
    let mut changed = output.clone();
    let bytes: [u8; 4] = changed
        .final_text_bytes
        .get(field..field + 4)
        .ok_or("native entry control call field is outside text")?
        .try_into()
        .map_err(|_| "native entry control call field has the wrong width")?;
    let word = u32::from_le_bytes(bytes);
    let replacement = match object.target().architecture {
        target::Architecture::Aarch64 => {
            (word & !0x03ff_ffff) | ((word & 0x03ff_ffff).wrapping_add(1) & 0x03ff_ffff)
        }
        target::Architecture::X86_64 => word.wrapping_add(1),
    };
    changed.final_text_bytes[field..field + 4].copy_from_slice(&replacement.to_le_bytes());
    let call = Some(
        u32::try_from(bridge.call_field_offset()).map_err(|_| "native call field exceeds u32")?,
    );
    let decode = |bytes: &[u8]| match object.target().architecture {
        target::Architecture::Aarch64 => isa_aarch64::decode_aarch64_generated_entry(bytes, call),
        target::Architecture::X86_64 => isa_x86_64::decode_x86_64_generated_entry(bytes, call),
    };
    if decode(original)? != decode(&changed.final_text_bytes[start..end])? {
        return Err("native call-target control changed more than the relocation value".into());
    }
    // Observe the unchanged text extent in the actual writer payload. The
    // complete reader checks the loader mapping before the call destination;
    // an ambiguous observation or a wrong file extent cannot pass this control.
    if output.final_text_bytes.is_empty() {
        return Err("native entry control needs a nonempty text extent".into());
    }
    let positions = output
        .bytes
        .windows(output.final_text_bytes.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == output.final_text_bytes).then_some(offset))
        .collect::<Vec<_>>();
    let [file_offset] = positions.as_slice() else {
        return Err("native entry control needs one exact mapped text observation".into());
    };
    let file_field = file_offset
        .checked_add(field)
        .ok_or("native entry file field overflows")?;
    changed.bytes[file_field..file_field + 4].copy_from_slice(&replacement.to_le_bytes());
    let regions = output
        .executable_regions
        .regions
        .iter()
        .map(|region| image::FinalExecutableRegion {
            origin: region.origin,
            section_offset: region.section_offset,
            byte_count: region.byte_count,
            symbol: region.symbol.clone(),
            footprint: region.footprint.clone(),
            entry_effects: region.entry_effects.clone(),
        })
        .collect();
    changed.executable_regions = image::place_executable_extent(
        &changed.final_text_bytes,
        regions,
        output.final_image_layout.text_address,
    )
    .map_err(|error| format!("native entry candidate inventory: {error}"))?;
    match image_emission::validate_direct_executable_image_output(object, retained, &changed) {
        Err(error)
            if error.message == "relative field does not resolve to its exact selected target" => {}
        Err(error) => {
            return Err(format!(
                "native call-target control rejected for an unrelated reason: {error}"
            ));
        }
        Ok(()) => return Err("native entry readback accepted a changed body-call target".into()),
    }
    // This source's callback body uses no vector state. The adapter must still
    // declare the vector save/restore it emits for the stronger inbound ABI.
    use calling_conventions::{MachineState as State, MachineStateSet};
    let vector = MachineStateSet::new([State::VectorRegisters]);
    if !bridge
        .generated()
        .effects
        .machine_state()
        .contains_all(vector)
        || bridge
            .request()
            .body_footprint
            .as_ref()
            .ok_or("native callback lost its body effects")?
            .machine_state()
            .contains_all(vector)
    {
        return Err(
            "native state control needs a scalar body with generated vector preservation".into(),
        );
    }
    let mut missing = bridge.request().clone();
    missing.inbound.state.permitted_transitive_use = missing
        .inbound
        .state
        .permitted_transitive_use
        .intersection(MachineStateSet::new([
            State::GeneralRegisters,
            State::Flags,
            State::InstructionPointer,
            State::StackPointer,
            State::ControlState,
            State::SegmentState,
        ]));
    match machine_emission::native_entry_bridge::emit_native_entry_bridge(missing) {
        Err(
            machine_emission::native_entry_bridge::NativeEntryBridgeError::GeneratedEntryEffects(
                reason,
            ),
        ) if reason.starts_with(
            "complete entry code uses machine state outside its declared ceiling:",
        ) => {}
        Err(error) => {
            return Err(format!(
                "generated vector control rejected for an unrelated reason: {error:?}"
            ));
        }
        Ok(_) => return Err("native entry accepted undeclared generated vector use".into()),
    }
    Ok(())
}
