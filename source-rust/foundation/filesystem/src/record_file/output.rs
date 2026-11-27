//! Replace a caller-selected output while keeping the old file intact until
//! the staged bytes are verified. The caller already owns the bytes to write.
use super::{
    CapabilityFile, RecordFileError, RecordFileLimits, RecordFileRoot, io_at, single_file_name,
    stage,
};
use sha2::{Digest, Sha256};
use std::io::{Read, Seek};
use std::path::Path;

impl RecordFileRoot {
    pub fn publish_output(&self, relative: &Path, bytes: &[u8]) -> Result<(), RecordFileError> {
        let name = single_file_name(relative)?;
        let path = self.display_path.join(name);
        let limits = RecordFileLimits {
            maximum_bytes: bytes.len(),
        };
        let Some(mut existing) = self.open_regular_optional(name, &path)? else {
            return self.write_new(relative, bytes, limits);
        };

        // A replacement may be much smaller than its predecessor. Retain
        // just the previous digest while the new output is being staged.
        let metadata = io_at(existing.metadata(), &path)?;
        let digest = file_digest(&mut existing, metadata.len(), &path)?;
        let staged = stage::prepare(self, bytes, limits, Some(metadata.permissions()))?;
        if file_digest(&mut existing, metadata.len(), &path)? != digest {
            return Err(RecordFileError::ContentsChanged { path });
        }
        self.verify_regular(name, &existing)?;
        stage::replace(self, staged, name, &path, bytes, limits)
    }
}

fn file_digest(
    file: &mut CapabilityFile,
    length: u64,
    path: &Path,
) -> Result<[u8; 32], RecordFileError> {
    io_at(file.rewind(), path)?;
    let mut hash = Sha256::new();
    let count = io_at(
        std::io::copy(&mut file.take(length.saturating_add(1)), &mut hash),
        path,
    )?;
    if count != length {
        return Err(RecordFileError::ContentsChanged {
            path: path.to_owned(),
        });
    }
    Ok(hash.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::temporary_directory::TemporaryDirectory;

    #[test]
    fn a_smaller_output_can_replace_a_large_existing_file() {
        let tmp = TemporaryDirectory::create("psi-replace-large-output", false).unwrap();
        let name = Path::new("game.psi");
        let path = tmp.path().join(name);
        let root = RecordFileRoot::open_directory(tmp.path()).unwrap();
        root.publish_output(name, b"previous").unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(64 * 1024 * 1024 + 1)
            .unwrap();
        root.publish_output(name, b"new").unwrap();
        assert_eq!(std::fs::read(path).unwrap(), b"new");
    }
}
