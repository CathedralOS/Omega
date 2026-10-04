//! A comparison replays one finite byte stream without a blocking pipe writer.
//! Scratch input is separate from retained build artifacts and always disposable.

use crate::temporary_directory::TemporaryDirectory;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};

pub(super) struct ReplayInput {
    bytes: Vec<u8>,
    directory: TemporaryDirectory,
}

impl ReplayInput {
    pub(super) fn capture(mut reader: impl Read) -> io::Result<Self> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        let directory = TemporaryDirectory::create("run-input", false)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options
            .open(directory.path().join("stdin.bin"))?
            .write_all(&bytes)?;
        Ok(Self { bytes, directory })
    }

    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(super) fn open_native_input(&self) -> io::Result<File> {
        File::open(self.directory.path().join("stdin.bin"))
    }
}
