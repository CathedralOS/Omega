use crate::pipeline::compile_options::CompileOptions;
use crate::pipeline::compile_policy::ExecutableTcbInstallationAuthorization;
use crate::pipeline::stages::EmittedProgram;
use omega_artifacts::ArtifactWriter;
use omega_image_emission::{
    ExecutableImageInput, can_emit_executable_image, emit_checked_executable_image,
};
use omega_object_file::{ObjectContainerInput, emit_omega_object_container};
use psi_diagnostics::Diagnostic;

pub(super) fn write_output(
    options: &CompileOptions,
    executable_tcb_authorization: &ExecutableTcbInstallationAuthorization,
    emitted: EmittedProgram,
    footprints: &omega_target_operations::BoundaryFootprintPlan,
) -> Result<std::path::PathBuf, Vec<Diagnostic>> {
    executable_tcb_authorization.authorize_installation();
    let build_dir = options.build_dir();
    std::fs::create_dir_all(&build_dir).map_err(io_diagnostic)?;

    if can_emit_executable_image(emitted.target) {
        let mut image = emit_checked_executable_image(
            ExecutableImageInput {
                target: emitted.target,
                object: &emitted.object,
                relocations: &emitted.relocations,
                encoded_machine_code: &emitted.encoded_machine_code,
                text_bytes: &emitted.text_bytes,
                data_bytes: &emitted.data_bytes,
                subsystem: emitted.subsystem,
            },
            emitted.planned_text_bytes,
        )
        .map_err(|diagnostic| vec![diagnostic])?;

        if footprints.boundary_contract_fingerprint.is_some() {
            omega_image::bind_compiler_entry_footprint(
                &mut image.executable_regions,
                omega_object_file::object_entry_symbol_name(&emitted.object),
                footprints.composed_evidence(),
            )
            .map_err(|diagnostic| vec![diagnostic])?;
        }

        let compiler_text_validation = image.compiler_text_validation.ok_or_else(|| {
            vec![Diagnostic::error(
                "checked executable image omitted compiler-text validation evidence",
            )]
        })?;
        let output_path = build_dir.join(&image.file_name);
        write_output_file(&output_path, &image.bytes, true)
            .map_err(|diagnostic| vec![diagnostic])?;
        write_executable_region_inventory(
            options,
            footprints,
            &compiler_text_validation,
            &image.executable_regions,
        )?;

        // The GUI-subsystem translation for Mach-O: PE stamps Subsystem 2 into
        // the image header so Windows never attaches a console box; macOS has no
        // header equivalent — a bare executable double-clicked in Finder routes
        // through Terminal. The equivalent is an `.app` bundle, so lay one out
        // beside the flat binary (which stays, for tests and terminal runs).
        // The embedded ad-hoc signature is content-hashed, so the copied
        // executable stays valid inside the bundle.
        if emitted.target.object_format == omega_target::ObjectFormat::MachO
            && emitted.subsystem == GUI_SUBSYSTEM
        {
            write_macos_app_bundle(options, &build_dir, &image.file_name, &image.bytes)?;
        }
        return Ok(output_path);
    }

    let object_container = emit_omega_object_container(ObjectContainerInput {
        target: emitted.target,
        object: &emitted.object,
        relocations: &emitted.relocations,
        text_bytes: &emitted.text_bytes,
        data_bytes: &emitted.data_bytes,
    });
    let output_path = build_dir.join(&object_container.file_name);
    write_output_file(&output_path, &object_container.bytes, false)
        .map_err(|diagnostic| vec![diagnostic])?;
    Ok(output_path)
}

fn write_executable_region_inventory(
    options: &CompileOptions,
    footprints: &omega_target_operations::BoundaryFootprintPlan,
    compiler_text_validation: &omega_image::CompilerTextValidationEvidence,
    inventory: &omega_image::PlacedExecutableRegionInventory,
) -> Result<(), Vec<Diagnostic>> {
    fn push_string(output: &mut String, value: &str) {
        output.push('"');
        for character in value.chars() {
            match character {
                '"' => output.push_str("\\\""),
                '\\' => output.push_str("\\\\"),
                '\n' => output.push_str("\\n"),
                '\r' => output.push_str("\\r"),
                '\t' => output.push_str("\\t"),
                character => output.push(character),
            }
        }
        output.push('"');
    }

    fn push_footprint(
        output: &mut String,
        footprint: Option<&omega_calling_conventions::StateFootprintEvidence>,
    ) {
        let Some(footprint) = footprint else {
            output.push_str("null");
            return;
        };
        output.push_str("{\"fingerprint\": ");
        push_string(
            output,
            &format!("0x{:016x}", footprint.evidence_fingerprint()),
        );
        output.push_str(&format!(
            ", \"machine_state_bits\": {}, \"registers\": [",
            footprint.machine_state().bits()
        ));
        for (index, register) in footprint.registers().as_slice().iter().enumerate() {
            if index > 0 {
                output.push_str(", ");
            }
            push_string(output, &format!("{register:?}"));
        }
        output.push_str("]}");
    }

    let implementation_evidence = footprints.composed_evidence();
    let implementation_evidence_fingerprint = implementation_evidence.evidence_fingerprint();
    let binding_fingerprint = boundary_placement_binding_fingerprint(
        footprints.boundary_contract_fingerprint,
        implementation_evidence_fingerprint,
        compiler_text_validation.derivation_fingerprint,
        inventory.inventory_fingerprint,
    );
    let mut json = String::from(
        "{\n  \"placement_stage\": \"final_image\",\n  \"enumeration_complete\": false,\n  \"region_enumeration_complete\": true,\n  \"footprint_enumeration_complete\": false,\n  \"covered_classes\": [\"compiler_functions\", \"import_thunks\"],\n  \"absent_by_construction_classes\": [\"relaxation_products\", \"veneers\", \"generated_stubs\"],\n  \"final_byte_validated_classes\": [\"compiler_function_relocation_envelope\", \"compiler_entry_call_return_mechanics\", \"catalog_checked_assembly\", \"import_thunks\"],\n  \"missing_classes\": [\"compiler_function_body_footprint_decoding\", \"admitted_leaves\"],\n",
    );
    json.push_str("  \"boundary_contract_fingerprint\": ");
    if let Some(fingerprint) = footprints.boundary_contract_fingerprint {
        push_string(&mut json, &format!("0x{fingerprint:016x}"));
    } else {
        json.push_str("null");
    }
    json.push_str(&format!(
        ",\n  \"implementation_evidence_fingerprint\": \"0x{implementation_evidence_fingerprint:016x}\",\n  \"implementation_fragment_count\": {},\n  \"compiler_text_validation\": {{\"encoded_text_fingerprint\": \"0x{:016x}\", \"final_compiler_text_fingerprint\": \"0x{:016x}\", \"relocation_envelope_fingerprint\": \"0x{:016x}\", \"checked_instruction_validation_fingerprint\": \"0x{:016x}\", \"derivation_fingerprint\": \"0x{:016x}\", \"text_relocation_count\": {}, \"checked_instruction_validation_count\": {}}},\n  \"inventory_fingerprint\": \"0x{:016x}\",\n  \"boundary_placement_binding_fingerprint\": \"0x{binding_fingerprint:016x}\",\n",
        footprints.fragments.len(),
        compiler_text_validation.encoded_text_fingerprint,
        compiler_text_validation.final_compiler_text_fingerprint,
        compiler_text_validation.relocation_envelope_fingerprint,
        compiler_text_validation.checked_instruction_validation_fingerprint,
        compiler_text_validation.derivation_fingerprint,
        compiler_text_validation.text_relocation_count,
        compiler_text_validation.checked_instruction_validation_count,
        inventory.inventory_fingerprint
    ));
    json.push_str(&format!(
        "  \"text_address\": \"0x{:016x}\",\n  \"text_byte_count\": {},\n  \"text_fingerprint\": \"0x{:016x}\",\n  \"regions\": [",
        inventory.text_address, inventory.text_byte_count, inventory.text_fingerprint
    ));
    for (index, region) in inventory.regions.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("\n    {\"origin\": ");
        push_string(
            &mut json,
            match region.origin {
                omega_image::FinalExecutableRegionOrigin::CompilerFunction => "compiler_function",
                omega_image::FinalExecutableRegionOrigin::ImportThunk => "import_thunk",
            },
        );
        json.push_str(", \"symbol\": ");
        push_string(&mut json, &region.symbol);
        json.push_str(&format!(
            ", \"section_offset\": {}, \"address\": \"0x{:016x}\", \"byte_count\": {}, \"byte_fingerprint\": \"0x{:016x}\", \"footprint\": ",
            region.section_offset, region.address, region.byte_count, region.byte_fingerprint
        ));
        push_footprint(&mut json, region.footprint.as_ref());
        json.push('}');
    }
    if !inventory.regions.is_empty() {
        json.push('\n');
        json.push_str("  ");
    }
    json.push_str("],\n  \"unclassified_gaps\": [");
    for (index, gap) in inventory.unclassified_gaps.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "\n    {{\"section_offset\": {}, \"address\": \"0x{:016x}\", \"byte_count\": {}, \"byte_fingerprint\": \"0x{:016x}\"}}",
            gap.section_offset, gap.address, gap.byte_count, gap.byte_fingerprint
        ));
    }
    if !inventory.unclassified_gaps.is_empty() {
        json.push('\n');
        json.push_str("  ");
    }
    json.push_str("]\n}\n");

    ArtifactWriter::new(&options.build_dir())
        .and_then(|writer| writer.write_text("13_executable_regions.json", &json))
        .map_err(|diagnostic| vec![diagnostic])
}

fn boundary_placement_binding_fingerprint(
    boundary_contract_fingerprint: Option<u64>,
    implementation_evidence_fingerprint: u64,
    compiler_text_derivation_fingerprint: u64,
    inventory_fingerprint: u64,
) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in [
        boundary_contract_fingerprint.is_some() as u8,
        0x42,
        0x50,
        0x42,
    ]
    .into_iter()
    .chain(
        boundary_contract_fingerprint
            .unwrap_or_default()
            .to_le_bytes(),
    )
    .chain(implementation_evidence_fingerprint.to_le_bytes())
    .chain(compiler_text_derivation_fingerprint.to_le_bytes())
    .chain(inventory_fingerprint.to_le_bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn io_diagnostic(error: std::io::Error) -> Vec<Diagnostic> {
    vec![Diagnostic::error(error.to_string())]
}

#[cfg(test)]
mod executable_region_inventory_tests {
    use super::boundary_placement_binding_fingerprint;

    #[test]
    fn placement_binding_changes_with_contract_evidence_or_final_inventory() {
        let baseline = boundary_placement_binding_fingerprint(Some(1), 2, 3, 4);
        assert_ne!(
            baseline,
            boundary_placement_binding_fingerprint(Some(5), 2, 3, 4)
        );
        assert_ne!(
            baseline,
            boundary_placement_binding_fingerprint(Some(1), 5, 3, 4)
        );
        assert_ne!(
            baseline,
            boundary_placement_binding_fingerprint(Some(1), 2, 6, 4)
        );
        assert_ne!(
            baseline,
            boundary_placement_binding_fingerprint(Some(1), 2, 3, 7)
        );
        assert_ne!(
            baseline,
            boundary_placement_binding_fingerprint(None, 2, 3, 4)
        );
    }
}

/// PE optional-header Subsystem word for a GUI program (`Subsystem::Gui`;
/// console is 3). Shared meaning across targets: the PE writer stamps it, the
/// Mach-O path translates it into an `.app` bundle.
const GUI_SUBSYSTEM: u16 = 2;

/// Lays out `build/<name>.app/Contents/{Info.plist,PkgInfo,MacOS/<exe>}` so a
/// Finder launch runs the program as a real windowed app (no Terminal). `<name>`
/// is the project directory name (e.g. `window_demo`).
fn write_macos_app_bundle(
    options: &CompileOptions,
    build_dir: &std::path::Path,
    executable_name: &str,
    executable_bytes: &[u8],
) -> Result<(), Vec<Diagnostic>> {
    let app_name: String = options
        .root_path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("omega-program")
        .chars()
        .map(|character| {
            // Keep the plist honest without an XML escaper: path characters that
            // are XML-significant or exotic collapse to '-'.
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | ' ' | '-') {
                character
            } else {
                '-'
            }
        })
        .collect();
    let bundle_identifier: String = app_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect();

    let contents_dir = build_dir.join(format!("{app_name}.app")).join("Contents");
    let macos_dir = contents_dir.join("MacOS");
    std::fs::create_dir_all(&macos_dir).map_err(io_diagnostic)?;

    let info_plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleExecutable</key>
	<string>{executable_name}</string>
	<key>CFBundleIdentifier</key>
	<string>org.omega-lang.{bundle_identifier}</string>
	<key>CFBundleName</key>
	<string>{app_name}</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>NSHighResolutionCapable</key>
	<true/>
</dict>
</plist>
"#
    );
    std::fs::write(contents_dir.join("Info.plist"), info_plist).map_err(io_diagnostic)?;
    std::fs::write(contents_dir.join("PkgInfo"), b"APPL????").map_err(io_diagnostic)?;
    write_output_file(&macos_dir.join(executable_name), executable_bytes, true)
        .map_err(|diagnostic| vec![diagnostic])?;
    Ok(())
}

fn write_output_file(
    output_path: &std::path::Path,
    bytes: &[u8],
    executable: bool,
) -> Result<(), Diagnostic> {
    let temp_path = output_path.with_file_name(format!(
        ".{}.{}.tmp",
        output_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("omega-output"),
        std::process::id()
    ));
    let _ = std::fs::remove_file(&temp_path);
    std::fs::write(&temp_path, bytes).map_err(|error| {
        Diagnostic::error(format!("failed to write {}: {error}", temp_path.display()))
    })?;

    if executable {
        mark_executable_if_needed(&temp_path)?;
    }

    match std::fs::rename(&temp_path, output_path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = std::fs::remove_file(&temp_path);
            Err(Diagnostic::error(format!(
                "failed to install {}: {error}",
                output_path.display()
            )))
        }
    }
}

#[cfg(unix)]
fn mark_executable_if_needed(path: &std::path::Path) -> Result<(), Diagnostic> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)
        .map_err(|error| Diagnostic::error(format!("failed to read {}: {error}", path.display())))?
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).map_err(|error| {
        Diagnostic::error(format!(
            "failed to mark {} executable: {error}",
            path.display()
        ))
    })
}

#[cfg(not(unix))]
fn mark_executable_if_needed(_path: &std::path::Path) -> Result<(), Diagnostic> {
    Ok(())
}
