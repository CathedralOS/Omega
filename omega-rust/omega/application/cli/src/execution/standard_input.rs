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

#[cfg(test)]
mod tests {
    use super::ReplayInput;
    use std::io::{self, Read};

    #[test]
    fn replay_retains_exact_bytes_and_removes_its_scratch_storage() {
        for bytes in [Vec::new(), vec![0, 255, 13, 10], vec![42; 131_072]] {
            let replay = ReplayInput::capture(bytes.as_slice()).unwrap();
            assert_eq!(replay.bytes(), bytes);
            let directory = replay.directory.path().to_path_buf();
            // Reopening starts at zero; no pipe capacity or text conversion is
            // involved, including for input larger than a typical pipe buffer.
            for _ in 0..2 {
                let mut actual = Vec::new();
                replay
                    .open_native_input()
                    .unwrap()
                    .read_to_end(&mut actual)
                    .unwrap();
                assert_eq!(actual, bytes);
            }
            drop(replay);
            assert!(!directory.exists());
        }
    }

    #[test]
    fn input_read_errors_are_not_replaced_with_empty_input() {
        struct FailedInput;
        impl Read for FailedInput {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            }
        }
        assert!(matches!(
            ReplayInput::capture(FailedInput),
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied
        ));
    }
}
