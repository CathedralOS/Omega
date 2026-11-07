// We resolve external-local dependency paths relative to the requester's
// canonical root and re-export cached local source recovery.

mod recovery;
pub(crate) use recovery::recover_cached_external_local_source;

use std::path::Path;

pub(crate) fn external_local_dependency_root(
    requester_root: &Path,
    location: &str,
) -> Result<std::path::PathBuf, String> {
    if location.is_empty() || location.bytes().any(|byte| byte.is_ascii_control()) {
        return Err("external-local path must be nonempty and contain no control bytes".into());
    }
    let authored = Path::new(location);
    Ok(if authored.is_absolute() {
        authored.to_path_buf()
    } else {
        requester_root.join(authored)
    })
}
