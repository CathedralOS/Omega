// Staging, ordered installation, and exact-byte replay of output files.
//
// Callers stage all members of a related output set (such as an artifact and
// its proof companion) beside their destinations and replay each staged file's
// bytes before installing any member. If installation or post-install replay
// fails partway through, `install_staged_files` unlinks the members already
// installed by this attempt so new artifact bytes never sit beside an older
// companion; untouched destinations remain intact, and overwritten destinations
// are not restored.
//
// `StagedFile` owns `bytes: Vec<u8>` so a staged set can outlive the scopes
// that produced each member's byte slice. The staging filename
// (`.<name>.<pid>.tmp`) is process-scoped, so callers must serialize
// publication to any shared destination path.

pub fn publish_exact_file_bytes(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let product = stage_exact_bytes(path, bytes, false)?;
    install_staged_files(std::slice::from_ref(&product))
}

pub struct StagedFile {
    staged: std::path::PathBuf,
    destination: std::path::PathBuf,
    bytes: Vec<u8>, // Owned for post-install replay after producer buffers drop.
}

pub fn stage_exact_bytes(
    path: &std::path::Path,
    bytes: &[u8],
    executable: bool,
) -> Result<StagedFile, String> {
    let parent = path
        .parent()
        .ok_or_else(|| "native publication path has no parent directory".to_owned())?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("omega-output");
    let staged = parent.join(format!(".{file_name}.{}.tmp", std::process::id()));
    let _ = std::fs::remove_file(&staged);
    std::fs::write(&staged, bytes)
        .map_err(|error| format!("failed to stage {}: {error}", staged.display()))?;
    let staged_bytes = std::fs::read(&staged)
        .map_err(|error| format!("failed to replay {}: {error}", staged.display()))?;
    if staged_bytes != bytes {
        let _ = std::fs::remove_file(&staged);
        return Err("staged native output bytes failed exact replay".to_owned());
    }
    if executable && let Err(error) = make_executable(&staged) {
        let _ = std::fs::remove_file(&staged);
        return Err(error);
    }
    Ok(StagedFile {
        staged,
        destination: path.to_path_buf(),
        bytes: bytes.to_vec(),
    })
}

fn replay_installed_product(product: &StagedFile) -> Result<(), String> {
    let installed = std::fs::read(&product.destination).map_err(|error| {
        format!(
            "failed to replay {}: {error}",
            product.destination.display()
        )
    })?;
    if installed != product.bytes {
        return Err("published native output bytes failed exact replay".to_owned());
    }
    Ok(())
}

pub fn install_staged_files(products: &[StagedFile]) -> Result<(), String> {
    let mut installed_count = 0;
    for product in products {
        let installation = install_staged(&product.staged, &product.destination).and_then(|()| {
            installed_count += 1;
            replay_installed_product(product)
        });
        if let Err(mut error) = installation {
            for installed_product in &products[..installed_count] {
                match std::fs::remove_file(&installed_product.destination) {
                    Ok(()) => {}
                    Err(cleanup_error) if cleanup_error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(cleanup_error) => error.push_str(&format!(
                        "; failed to remove incomplete publication {}: {cleanup_error}",
                        installed_product.destination.display()
                    )),
                }
            }
            return Err(error);
        }
    }
    Ok(())
}

#[cfg(unix)]
fn install_staged(staged: &std::path::Path, path: &std::path::Path) -> Result<(), String> {
    std::fs::rename(staged, path)
        .map_err(|error| format!("failed to publish {}: {error}", path.display()))
}

#[cfg(not(unix))]
fn install_staged(staged: &std::path::Path, path: &std::path::Path) -> Result<(), String> {
    if path.exists() {
        std::fs::remove_file(path)
            .map_err(|error| format!("failed to replace {}: {error}", path.display()))?;
    }
    std::fs::rename(staged, path)
        .map_err(|error| format!("failed to publish {}: {error}", path.display()))
}

#[cfg(unix)]
pub fn make_executable(path: &std::path::Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("failed to make {} executable: {error}", path.display()))
}

#[cfg(not(unix))]
pub fn make_executable(_path: &std::path::Path) -> Result<(), String> {
    Ok(())
}
