//! A comparison replays one finite byte stream without a blocking pipe writer.
//! Scratch input is separate from retained build artifacts and always disposable.

use platform_custody::temporary_directory::TemporaryDirectory;
use std::fs::{File, OpenOptions};
use std::io::{self, Read};

pub(super) struct ReplayInput {
    directory: TemporaryDirectory,
}

impl ReplayInput {
    pub(super) fn capture(mut reader: impl Read) -> io::Result<Self> {
        let directory = TemporaryDirectory::create("run-input", false)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut snapshot = options.open(directory.path().join("stdin.bin"))?;
        io::copy(&mut reader, &mut snapshot)?;
        Ok(Self { directory })
    }

    /// Each engine starts at byte zero with an independent stream position.
    pub(super) fn open_input(&self) -> io::Result<File> {
        File::open(self.directory.path().join("stdin.bin"))
    }
}
