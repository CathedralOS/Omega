//! Fail-closed publication for one newly validated artifact.

use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
};

use crate::error::OfflinePolicyCommandError;

pub(super) fn publish_new(path: &Path, encoded: &[u8]) -> Result<(), OfflinePolicyCommandError> {
    publish_with(path, |output| {
        output.write_all(encoded)?;
        output.sync_all()
    })
}

pub(super) fn publish_with(
    path: &Path,
    publish: impl FnOnce(&mut File) -> std::io::Result<()>,
) -> Result<(), OfflinePolicyCommandError> {
    let mut created = false;
    let result = (|| {
        let mut output = OpenOptions::new().write(true).create_new(true).open(path)?;
        created = true;
        publish(&mut output)
    })();

    if let Err(source) = result {
        if created {
            let _ = fs::remove_file(path);
        }
        return Err(OfflinePolicyCommandError::Publish {
            path: path.to_path_buf(),
            source,
        });
    }
    Ok(())
}
