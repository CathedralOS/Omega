use crate::pipeline::compile_options::CompileOptions;
use crate::pipeline::stages::EmittedProgram;
use omega_core::diagnostics::Diagnostic;
use omega_image_emission::{
    ExecutableImageInput, can_emit_executable_image, emit_checked_executable_image,
};
use omega_object_file::{ObjectContainerInput, emit_omega_object_container};

pub(super) fn write_output(
    options: &CompileOptions,
    emitted: EmittedProgram,
) -> Result<std::path::PathBuf, Vec<Diagnostic>> {
    let build_dir = options.build_dir();
    std::fs::create_dir_all(&build_dir).map_err(io_diagnostic)?;

    if can_emit_executable_image(emitted.target) {
        let image = emit_checked_executable_image(
            ExecutableImageInput {
                target: emitted.target,
                object: &emitted.object,
                relocations: &emitted.relocations,
                text_bytes: &emitted.text_bytes,
                data_bytes: &emitted.data_bytes,
                subsystem: emitted.subsystem,
            },
            emitted.planned_text_bytes,
        )
        .map_err(|diagnostic| vec![diagnostic])?;

        let output_path = build_dir.join(&image.file_name);
        write_output_file(&output_path, &image.bytes, true)
            .map_err(|diagnostic| vec![diagnostic])?;

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

fn io_diagnostic(error: std::io::Error) -> Vec<Diagnostic> {
    vec![Diagnostic::error(error.to_string())]
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
