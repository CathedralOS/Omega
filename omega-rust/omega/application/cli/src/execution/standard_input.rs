// Snapshot caller `stdin` into a private disposable file (`stdin.bin`) so `--both` can give Terminal Psi and the native child independent file descriptors positioned at byte 0 without a background pipe-writer thread.

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

    pub(super) fn open_input(&self) -> io::Result<File> {
        File::open(self.directory.path().join("stdin.bin"))
    }
}
