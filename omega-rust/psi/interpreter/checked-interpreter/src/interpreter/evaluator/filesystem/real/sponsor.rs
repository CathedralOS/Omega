use super::host_io::{open_options_for, open_real, win32_error_code};
use super::{EACCES, EBADF, io_errno, real_fs};
use crate::interpreter::evaluator::EvalResult;
use std::io::Seek;
use std::path::{Path, PathBuf};

pub(super) fn sponsor_value<T>(result: Result<T, crate::FilesystemSponsorError>) -> EvalResult<T> {
    match result {
        Ok(value) => Ok(value),
        Err(error) => crate::interpreter::evaluator::filesystem_sponsor_halt(error),
    }
}

pub(super) fn checked_written_count(written: usize) -> EvalResult<i64> {
    match i64::try_from(written) {
        Ok(written) => Ok(written),
        Err(_) => crate::interpreter::evaluator::filesystem_sponsor_halt(
            crate::FilesystemSponsorError::ArithmeticOverflow,
        ),
    }
}

/// Staging growth is reserved before host mutation. Unsponsored operations
/// retain ordinary host behavior; a staging protocol or resource refusal
/// stops before the operation, without replaying a duplicate namespace.
#[derive(Debug)]
pub(super) enum SponsorPreparation<T> {
    Unsponsored,
    Prepared(T),
}

pub(super) fn sponsor_preparation<T>(
    result: Result<T, crate::FilesystemSponsorError>,
) -> EvalResult<SponsorPreparation<T>> {
    sponsor_value(result).map(SponsorPreparation::Prepared)
}

pub(super) fn read_only_open_bypasses_sponsor(
    path: &Path,
    session_root: &Path,
    may_create: bool,
    truncates: bool,
    wants_write: bool,
) -> bool {
    (!path.starts_with(session_root) || path == session_root)
        && !wants_write
        && !may_create
        && !truncates
}

impl<'program> crate::interpreter::evaluator::Evaluator<'program> {
    /// The mounted sponsor and `fd`'s sponsor descriptor, when both exist;
    /// `None` is the unsponsored path every preparation shares.
    fn sponsor_context(
        &mut self,
        fd: i32,
    ) -> Option<(crate::FilesystemSponsor, crate::FilesystemOpenDescriptor)> {
        let real = self.real_fs_mut();
        real.sponsor.clone().and_then(|sponsor| {
            real.files
                .get(&fd)
                .and_then(|entry| entry.sponsor_descriptor)
                .map(|descriptor| (sponsor, descriptor))
        })
    }

    /// Enforce the compiler grant retained by an opened descriptor before a
    /// descriptor-based mutation reaches sponsor accounting or the host.
    /// Missing descriptors preserve each ABI family's native error shape.
    pub(super) fn require_real_descriptor_write_grant(
        &mut self,
        fd: i32,
        win32_errors: bool,
    ) -> bool {
        match real_fs(self).and_then(|filesystem| filesystem.descriptor_write_granted(fd)) {
            Some(true) => true,
            Some(false) => {
                self.real_fs_mut().errno = if win32_errors { 5 } else { EACCES };
                false
            }
            None => {
                self.real_fs_mut().errno = if win32_errors { 6 } else { EBADF };
                false
            }
        }
    }

    pub(super) fn prepare_sponsored_open(
        &mut self,
        path: &Path,
        may_create: bool,
        truncates: bool,
        wants_write: bool,
    ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemOpen>> {
        let Some(sponsor) = real_fs(self).and_then(|filesystem| filesystem.sponsor.clone()) else {
            return Ok(SponsorPreparation::Unsponsored);
        };
        let session_root = sponsor_value(sponsor.session_root())?;
        if read_only_open_bypasses_sponsor(path, &session_root, may_create, truncates, wants_write)
        {
            return Ok(SponsorPreparation::Unsponsored);
        }
        let sponsored_path = sponsor_value(sponsor.bind_path(path))?;
        if !wants_write && !may_create && !truncates {
            sponsor_value(sponsor.require_sealed(&sponsored_path))?;
            return Ok(SponsorPreparation::Unsponsored);
        }
        if !may_create {
            return sponsor_value(Err(
                crate::FilesystemSponsorError::UnsupportedStagingOperation(
                    "write-open of an existing file",
                ),
            ));
        }
        sponsor_preparation(sponsor.prepare_create_object_open(&sponsored_path, 0))
    }

    pub(super) fn finish_real_open(
        &mut self,
        opened: std::io::Result<std::fs::File>,
        path: PathBuf,
        append: bool,
        prepared: SponsorPreparation<crate::PreparedFilesystemOpen>,
        win32_errors: bool,
    ) -> EvalResult<i64> {
        match opened {
            Ok(file) => {
                let sponsor_descriptor = self.commit_sponsored_open(prepared)?;
                match self
                    .real_fs_mut()
                    .insert(file, path, sponsor_descriptor, append)
                {
                    Ok(fd) => Ok(fd),
                    Err(errno) => {
                        self.real_fs_mut().errno = if win32_errors { 5 } else { errno };
                        Ok(-1)
                    }
                }
            }
            Err(error) => {
                self.real_fs_mut().errno = if win32_errors {
                    win32_error_code(&error)
                } else {
                    io_errno(&error)
                };
                Ok(-1)
            }
        }
    }

    /// Every flag-decoded path open ends the same way: sponsor preparation,
    /// shared option decode, the real open, then finish. Call sites differ
    /// only in the `OpenOptions` mode arguments.
    pub(super) fn open_flags_and_finish(
        &mut self,
        path: PathBuf,
        flags: i32,
        mode: u32,
        apply_creation_mode: bool,
        wants_write: bool,
    ) -> EvalResult<i64> {
        let prepared = self.prepare_sponsored_open(
            &path,
            crate::interpreter::evaluator::filesystem::host_open_flags::o_creat(flags),
            crate::interpreter::evaluator::filesystem::host_open_flags::o_trunc(flags),
            wants_write,
        )?;
        let mut options = open_options_for(flags, mode, apply_creation_mode);
        if matches!(prepared, SponsorPreparation::Prepared(_)) {
            options.create_new(true);
        }
        let opened = open_real(&options, &path, wants_write);
        self.finish_real_open(
            opened,
            path,
            crate::interpreter::evaluator::filesystem::host_open_flags::o_append(flags),
            prepared,
            false,
        )
    }

    pub(super) fn commit_sponsored_open(
        &mut self,
        prepared: SponsorPreparation<crate::PreparedFilesystemOpen>,
    ) -> EvalResult<Option<crate::FilesystemOpenDescriptor>> {
        match prepared {
            SponsorPreparation::Unsponsored => Ok(None),
            SponsorPreparation::Prepared(prepared) => Ok(Some(sponsor_value(prepared.commit())?)),
        }
    }

    pub(super) fn prepare_sponsored_duplicate(
        &mut self,
        fd: i32,
    ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemOpen>> {
        let Some((sponsor, descriptor)) = self.sponsor_context(fd) else {
            return Ok(SponsorPreparation::Unsponsored);
        };
        let _ = (sponsor, descriptor);
        sponsor_value(Err(
            crate::FilesystemSponsorError::UnsupportedStagingOperation("duplicate writer"),
        ))
    }

    pub(super) fn prepare_sponsored_close(
        &mut self,
        fd: i32,
    ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemMutation>> {
        let Some((sponsor, descriptor)) = self.sponsor_context(fd) else {
            return Ok(SponsorPreparation::Unsponsored);
        };
        sponsor_preparation(sponsor.prepare_close(&descriptor))
    }

    pub(super) fn prepare_sponsored_set_extent(
        &mut self,
        fd: i32,
        extent: u64,
    ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemMutation>> {
        let Some((sponsor, descriptor)) = self.sponsor_context(fd) else {
            return Ok(SponsorPreparation::Unsponsored);
        };
        let _ = (sponsor, descriptor, extent);
        sponsor_value(Err(
            crate::FilesystemSponsorError::UnsupportedStagingOperation("truncate writer"),
        ))
    }

    pub(super) fn prepare_sponsored_write(
        &mut self,
        fd: i32,
        requested_bytes: usize,
        positioned_offset: Option<i64>,
    ) -> EvalResult<Result<SponsorPreparation<crate::PreparedFilesystemWrite>, i32>> {
        let real = self.real_fs_mut();
        let Some(sponsor) = real.sponsor.clone() else {
            return Ok(Ok(SponsorPreparation::Unsponsored));
        };
        let Some(entry) = real.files.get_mut(&fd) else {
            return Ok(Err(EBADF));
        };
        let Some(descriptor) = entry.sponsor_descriptor else {
            return Ok(Ok(SponsorPreparation::Unsponsored));
        };
        let offset = if entry.append {
            match entry.file.metadata() {
                Ok(metadata) => metadata.len(),
                Err(error) => return Ok(Err(io_errno(&error))),
            }
        } else if let Some(offset) = positioned_offset {
            u64::try_from(offset.max(0)).expect("nonnegative i64 fits in u64")
        } else {
            match entry.file.stream_position() {
                Ok(offset) => offset,
                Err(error) => return Ok(Err(io_errno(&error))),
            }
        };
        let requested_bytes = match u64::try_from(requested_bytes) {
            Ok(bytes) => bytes,
            Err(_) => {
                return crate::interpreter::evaluator::filesystem_sponsor_halt(
                    crate::FilesystemSponsorError::ArithmeticOverflow,
                );
            }
        };
        Ok(Ok(sponsor_preparation(sponsor.prepare_write(
            &descriptor,
            offset,
            requested_bytes,
        ))?))
    }

    pub(super) fn commit_sponsored_write(
        &mut self,
        prepared: SponsorPreparation<crate::PreparedFilesystemWrite>,
        written: usize,
    ) -> EvalResult<()> {
        let prepared = match prepared {
            SponsorPreparation::Unsponsored => return Ok(()),
            SponsorPreparation::Prepared(prepared) => prepared,
        };
        let written = match u64::try_from(written) {
            Ok(written) => written,
            Err(_) => {
                return crate::interpreter::evaluator::filesystem_sponsor_halt(
                    crate::FilesystemSponsorError::ArithmeticOverflow,
                );
            }
        };
        sponsor_value(prepared.commit_written(written))
    }

    pub(super) fn prepare_sponsored_create_directory(
        &mut self,
        path: &Path,
    ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemMutation>> {
        let Some((sponsor, path)) = self.sponsor_path(path)? else {
            return Ok(SponsorPreparation::Unsponsored);
        };
        sponsor_preparation(sponsor.prepare_create_directory(&path))
    }

    pub(super) fn prepare_sponsored_unlink(
        &mut self,
        path: &Path,
    ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemMutation>> {
        let Some((sponsor, path)) = self.sponsor_path(path)? else {
            return Ok(SponsorPreparation::Unsponsored);
        };
        let _ = (sponsor, path);
        sponsor_value(Err(
            crate::FilesystemSponsorError::UnsupportedStagingOperation("remove staging entry"),
        ))
    }

    pub(super) fn prepare_sponsored_rename(
        &mut self,
        from: &Path,
        to: &Path,
    ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemMutation>> {
        let Some(sponsor) = real_fs(self).and_then(|filesystem| filesystem.sponsor.clone()) else {
            return Ok(SponsorPreparation::Unsponsored);
        };
        sponsor_value(sponsor.bind_path(from))?;
        sponsor_value(sponsor.bind_path(to))?;
        sponsor_value(Err(
            crate::FilesystemSponsorError::UnsupportedStagingOperation("rename staging entry"),
        ))
    }

    pub(super) fn prepare_sponsored_hard_link(
        &mut self,
        existing: &Path,
        new_name: &Path,
    ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemMutation>> {
        let Some(sponsor) = real_fs(self).and_then(|filesystem| filesystem.sponsor.clone()) else {
            return Ok(SponsorPreparation::Unsponsored);
        };
        sponsor_value(sponsor.bind_path(existing))?;
        sponsor_value(sponsor.bind_path(new_name))?;
        sponsor_value(Err(
            crate::FilesystemSponsorError::UnsupportedStagingOperation("hard link staging entry"),
        ))
    }

    pub(super) fn prepare_sponsored_symlink(
        &mut self,
        link: &Path,
        target_spelling: &[u8],
    ) -> EvalResult<SponsorPreparation<crate::PreparedFilesystemMutation>> {
        let Some((sponsor, link)) = self.sponsor_path(link)? else {
            return Ok(SponsorPreparation::Unsponsored);
        };
        let _ = (sponsor, link, target_spelling);
        sponsor_value(Err(
            crate::FilesystemSponsorError::UnsupportedStagingOperation("symlink staging entry"),
        ))
    }

    pub(super) fn sponsor_path(
        &mut self,
        path: &Path,
    ) -> EvalResult<Option<(crate::FilesystemSponsor, crate::FilesystemSponsorPath)>> {
        let Some(sponsor) = real_fs(self).and_then(|filesystem| filesystem.sponsor.clone()) else {
            return Ok(None);
        };
        let path = sponsor_value(sponsor.bind_path(path))?;
        Ok(Some((sponsor, path)))
    }

    pub(super) fn commit_sponsored_mutation(
        &mut self,
        prepared: SponsorPreparation<crate::PreparedFilesystemMutation>,
    ) -> EvalResult<()> {
        match prepared {
            SponsorPreparation::Unsponsored => Ok(()),
            SponsorPreparation::Prepared(prepared) => sponsor_value(prepared.commit()),
        }
    }

    pub(super) fn finish_real_mutation(
        &mut self,
        outcome: std::io::Result<()>,
        prepared: SponsorPreparation<crate::PreparedFilesystemMutation>,
        win32_result: bool,
    ) -> EvalResult<i64> {
        match outcome {
            Ok(()) => {
                self.commit_sponsored_mutation(prepared)?;
                Ok(if win32_result { 1 } else { 0 })
            }
            Err(error) => {
                self.real_fs_mut().errno = if win32_result {
                    win32_error_code(&error)
                } else {
                    io_errno(&error)
                };
                Ok(if win32_result { 0 } else { -1 })
            }
        }
    }
}
