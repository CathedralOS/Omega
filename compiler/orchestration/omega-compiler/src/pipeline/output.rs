use crate::pipeline::compile_options::CompileOptions;
use crate::pipeline::stages::EmittedProgram;
use omega_core::diagnostics::Diagnostic;
use omega_image_emission::{
    ExecutableImageInput, can_emit_executable_image, emit_checked_executable_image,
};
use omega_object::{ObjectContainerInput, emit_omega_object_container};

pub(super) fn write_output(
    options: &CompileOptions,
    emitted: EmittedProgram,
) -> Result<(), Vec<Diagnostic>> {
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
            },
            emitted.planned_text_bytes,
        )
        .map_err(|diagnostic| vec![diagnostic])?;

        let output_path = build_dir.join(&image.file_name);
        write_output_file(&output_path, &image.bytes, true)
            .map_err(|diagnostic| vec![diagnostic])?;
        return Ok(());
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
    Ok(())
}

fn io_diagnostic(error: std::io::Error) -> Vec<Diagnostic> {
    vec![Diagnostic::error(error.to_string())]
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
