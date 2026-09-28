//! Install exactly the completed file roster, never scratch or generated source.
use crate::RetainedBuildOutputs;
use std::path::{Path, PathBuf};

/// Materialize a content-addressed set; the manifest is installed last.
/// Existing sets are checked against retained bytes, never adopted by name.
/// Failure may leave a partial copy but returns no successful publication.
pub(super) fn publish(outputs: &RetainedBuildOutputs, build_dir: &Path) -> Result<PathBuf, String> {
    let name = outputs
        .identity()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let sets = build_dir.join("completed");
    std::fs::create_dir_all(&sets)
        .map_err(|error| format!("cannot create completed-output directory: {error}"))?;
    let directory = sets.join(name);
    let fresh = match std::fs::create_dir(&directory) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
        Err(error) => return Err(format!("cannot create completed-output set: {error}")),
    };
    let files = directory.join("files");
    let manifest = directory.join("manifest.bin");
    if fresh {
        std::fs::create_dir(&files)
            .map_err(|error| format!("cannot create completed-output files: {error}"))?;
        outputs
            .files()
            .materialize_into(&files)
            .map_err(|error| error.to_string())?;
        platform_custody::file_publication::publish_exact_file_bytes(
            &manifest,
            outputs.manifest_bytes(),
        )?;
    } else {
        let metadata = std::fs::symlink_metadata(&directory).map_err(|error| error.to_string())?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err("completed-output set is not a concrete directory".into());
        }
        let metadata = std::fs::symlink_metadata(&manifest).map_err(|error| error.to_string())?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || std::fs::read(&manifest).map_err(|error| error.to_string())?
                != outputs.manifest_bytes()
        {
            return Err("completed-output manifest differs from this compilation".into());
        }
        outputs
            .files()
            .verify_materialized_at(&files)
            .map_err(|error| error.to_string())?;
    }
    Ok(directory)
}
