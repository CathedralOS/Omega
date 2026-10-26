//! One captured-input and append-and-seal Output activation.
//!
//! Interpreter adapters supply evaluated operands and retain their own opaque
//! language markers. This owner performs the operations, meters live handles
//! and observations, and records actual output settlements. It consumes the
//! admitted scope directly; no checked source or evaluator values enter here.

use super::{BUILD_OUTPUT_ROOT_IDENTITY, BUILD_SOURCE_ROOT_IDENTITY, BuildMachineFilesystemScope};
use crate::build_execution::observations::{
    BuildFilesystemAuthorizedPath, BuildFilesystemOperationAttempt, BuildFilesystemRoot,
    BuildIncludedSourceHandoff,
};
use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};
use filesystem_authority::{
    BuildOutputObligation, BuildOutputObligationState, BuildOutputReceipt, FilesystemGrantAccess,
    FilesystemGrantRootIdentity, FilesystemHostOperation, FilesystemLogicalHandleIdentity,
    FilesystemLogicalHandleInput, FilesystemLogicalHandleInputResolution,
    FilesystemLogicalHandleKind, FilesystemLogicalHandleOutput,
    FilesystemLogicalHandleOutputSource, FilesystemObservationProvider, FilesystemOpenDescriptor,
    FilesystemOperationResult, filesystem_root_relative_path_is_canonical,
};
use std::io::Write;

/// Observations belong to one completed activation. Only this runtime can
/// construct them; package settlement still validates the staged files.
pub struct BuildFilesystemEffects {
    pub(crate) scope: BuildMachineFilesystemScope,
    pub(crate) attempts: Vec<BuildFilesystemOperationAttempt>,
    pub(crate) included: Vec<BuildIncludedSourceHandoff>,
    pub(crate) obligations: Vec<BuildOutputObligation>,
    pub(crate) receipts: Vec<BuildOutputReceipt>,
    pub(crate) storage: Vec<EvaluationStorageLease>,
}

struct OpenFile {
    root: FilesystemGrantRootIdentity,
    path: Vec<u8>,
    storage: FileStorage,
    _handle: EvaluationStorageLease,
}

enum FileStorage {
    Captured {
        cursor: u64,
    },
    Output {
        file: std::fs::File,
        writer: FilesystemOpenDescriptor,
        extent: u64,
    },
}

pub struct BuildFilesystemRuntime<'scope> {
    scope: &'scope BuildMachineFilesystemScope,
    account: EvaluationInvocationAccount,
    descriptors: Vec<Option<OpenFile>>,
    effects: BuildFilesystemEffects,
    sealed: Vec<(Vec<u8>, usize)>,
    incomplete_attempt: Option<usize>,
    refused: Option<String>,
}

impl BuildMachineFilesystemScope {
    /// Establish exactly the already-admitted captured roots and private
    /// Output staging. Live-host protocols remain a separate authority mode.
    pub fn begin_filesystem_runtime(
        &self,
        account: EvaluationInvocationAccount,
    ) -> Result<BuildFilesystemRuntime<'_>, String> {
        if self.captured_source_view.is_none() {
            return Err("Terminal Build requires admitted captured Source storage".into());
        }
        // Build admission already established the write fence before consent.
        // Borrow that exact scope instead of running admission a second time.
        let access = self.filesystem_access();
        if !self.is_private_snapshot_execution(&access) {
            return Err("Build Output is not owned by its private staging sponsor".into());
        }
        Ok(BuildFilesystemRuntime {
            scope: self,
            account,
            descriptors: Vec::new(),
            effects: BuildFilesystemEffects {
                scope: self.clone(),
                attempts: Vec::new(),
                included: Vec::new(),
                obligations: Vec::new(),
                receipts: Vec::new(),
                storage: Vec::new(),
            },
            sealed: Vec::new(),
            incomplete_attempt: None,
            refused: None,
        })
    }
}

impl BuildFilesystemRuntime<'_> {
    pub fn named_input(&self, name: &[u8]) -> Result<FilesystemGrantRootIdentity, String> {
        self.scope
            .named_inputs
            .iter()
            .find(|input| input.name == name)
            .map(|input| input.root)
            .ok_or_else(|| "Build named input was not admitted".into())
    }

    pub fn resolve(
        &self,
        root: FilesystemGrantRootIdentity,
        relative: &[u8],
    ) -> Result<(), String> {
        if !filesystem_root_relative_path_is_canonical(relative, false) {
            return Err("Build path is not a canonical relative path".into());
        }
        if root != BUILD_OUTPUT_ROOT_IDENTITY {
            self.snapshot(root)?;
        }
        Ok(())
    }

    fn snapshot(
        &self,
        root: FilesystemGrantRootIdentity,
    ) -> Result<&source::SourceSnapshot, String> {
        if root == BUILD_SOURCE_ROOT_IDENTITY {
            return self
                .scope
                .captured_source_view
                .as_ref()
                .ok_or_else(|| "Source capture is absent".into());
        }
        self.scope
            .named_inputs
            .iter()
            .find(|input| input.root == root)
            .map(|input| &input.snapshot)
            .ok_or_else(|| "unknown Build input root".into())
    }

    fn begin_attempt(
        &mut self,
        operation: FilesystemHostOperation,
        root: FilesystemGrantRootIdentity,
        path: &[u8],
        access: FilesystemGrantAccess,
        descriptor: Option<i32>,
    ) -> Result<usize, String> {
        if self.incomplete_attempt.is_some() || self.refused.is_some() {
            return Err("Build filesystem activation has already failed".into());
        }
        self.account
            .charge_filesystem_operation_attempt()
            .map_err(|error| error.to_string())?;
        let storage = self
            .account
            .reserve_site(
                0,
                3 + 2 * u64::from(descriptor.is_some()),
                path.len() as u64,
            )
            .map_err(|error| error.to_string())?;
        let meaning = if root == BUILD_SOURCE_ROOT_IDENTITY {
            BuildFilesystemRoot::Source
        } else if root == BUILD_OUTPUT_ROOT_IDENTITY {
            BuildFilesystemRoot::Output
        } else {
            self.snapshot(root)?;
            BuildFilesystemRoot::NamedInput(root.get())
        };
        let mut input = Vec::new();
        let mut retired_logical_handles = Vec::new();
        if let Some(descriptor) = descriptor {
            input
                .try_reserve(1)
                .map_err(|_| "cannot retain filesystem handle operand")?;
            retired_logical_handles
                .try_reserve(1)
                .map_err(|_| "cannot retain filesystem handle retirement")?;
            input.push(FilesystemLogicalHandleInput::new(
                0,
                FilesystemLogicalHandleKind::Descriptor,
                FilesystemLogicalHandleInputResolution::Resolved(logical_handle(descriptor)?),
            ));
        }
        let mut authorized_paths = Vec::new();
        let mut grant_refusals = Vec::new();
        grant_refusals
            .try_reserve(1)
            .map_err(|_| "cannot retain filesystem grant refusal")?;
        authorized_paths
            .try_reserve(1)
            .map_err(|_| "cannot retain authorized filesystem path")?;
        authorized_paths.push(BuildFilesystemAuthorizedPath {
            operand_ordinal: 0,
            access,
            root: meaning,
            relative_path: copy_bytes(path)?,
        });
        let ordinal = self.effects.attempts.len();
        self.effects
            .attempts
            .try_reserve(1)
            .map_err(|_| "cannot retain filesystem attempt")?;
        self.effects
            .storage
            .try_reserve(1)
            .map_err(|_| "cannot retain filesystem reservation")?;
        self.effects.attempts.push(BuildFilesystemOperationAttempt {
            operation_tag: operation.operation_tag(),
            provider: if root == BUILD_OUTPUT_ROOT_IDENTITY {
                FilesystemObservationProvider::RealScoped
            } else {
                FilesystemObservationProvider::Captured
            },
            result: FilesystemOperationResult::Scalar(-1),
            post_error: 0,
            authorized_paths,
            logical_handle_inputs: input,
            logical_handle_output: None,
            retired_logical_handles,
            grant_refusals,
        });
        self.effects.storage.push(storage);
        self.incomplete_attempt = Some(ordinal);
        Ok(ordinal)
    }

    fn complete_attempt(&mut self, ordinal: usize, result: i64, error: i32) -> i64 {
        self.effects.attempts[ordinal].result = FilesystemOperationResult::Scalar(result);
        self.effects.attempts[ordinal].post_error = error;
        self.incomplete_attempt = None;
        result
    }

    pub fn open(
        &mut self,
        root: FilesystemGrantRootIdentity,
        path: &[u8],
        flags: i32,
    ) -> Result<i32, String> {
        self.resolve(root, path)?;
        if root == BUILD_OUTPUT_ROOT_IDENTITY {
            return Err("BuildSource cannot open the Output root".into());
        }
        let ordinal = self.begin_attempt(
            FilesystemHostOperation::Open,
            root,
            path,
            FilesystemGrantAccess::Read,
            None,
        )?;
        if flags != 0 {
            let wants_write =
                filesystem_authority::open_flags::wants_write(flags) || flags & 3 != 0;
            if wants_write {
                let attempt = &mut self.effects.attempts[ordinal];
                attempt.authorized_paths.clear();
                attempt
                    .grant_refusals
                    .push(filesystem_authority::FilesystemGrantRefusal::new(
                        0,
                        FilesystemGrantAccess::Write,
                        filesystem_authority::FilesystemGrantRefusalReason::OutsideGrantedRoots,
                    ));
            }
            return Ok(
                self.complete_attempt(ordinal, -1, if wants_write { 13 } else { 45 }) as i32,
            );
        }
        match self.snapshot(root)?.lookup(path) {
            Ok(source::SourceSnapshotEntry::File(_)) => {}
            Ok(source::SourceSnapshotEntry::Directory) => {
                return Ok(self.complete_attempt(ordinal, -1, 21) as i32);
            }
            Ok(source::SourceSnapshotEntry::Symlink(_)) => {
                return Ok(self.complete_attempt(ordinal, -1, 40) as i32);
            }
            _ => return Ok(self.complete_attempt(ordinal, -1, 2) as i32),
        }
        let handle = self
            .account
            .reserve_site(1, 1, path.len() as u64)
            .map_err(|error| error.to_string())?;
        let descriptor = i32::try_from(self.descriptors.len())
            .ok()
            .and_then(|value| value.checked_add(3))
            .ok_or("Build descriptor space exhausted")?;
        self.descriptors
            .try_reserve(1)
            .map_err(|_| "cannot retain Build descriptor")?;
        self.descriptors.push(Some(OpenFile {
            root,
            path: copy_bytes(path)?,
            storage: FileStorage::Captured { cursor: 0 },
            _handle: handle,
        }));
        self.effects.attempts[ordinal].logical_handle_output =
            Some(FilesystemLogicalHandleOutput::new(
                FilesystemLogicalHandleKind::Descriptor,
                logical_handle(descriptor)?,
                FilesystemLogicalHandleOutputSource::Created,
            ));
        self.effects.attempts[ordinal].result =
            FilesystemOperationResult::LogicalHandle(logical_handle(descriptor)?);
        self.incomplete_attempt = None;
        Ok(descriptor)
    }

    pub fn read(
        &mut self,
        root: FilesystemGrantRootIdentity,
        descriptor: i32,
        buffer: &mut [u8],
        count: u64,
    ) -> Result<i64, String> {
        let slot = self.descriptor_slot(root, descriptor)?;
        let file = self.descriptors[slot]
            .as_ref()
            .ok_or("closed Build descriptor")?;
        let FileStorage::Captured { cursor } = &file.storage else {
            return Err("BuildSource read requires an input descriptor".into());
        };
        let cursor = *cursor;
        let (path, _path_storage) = self.descriptor_path(slot)?;
        if count > buffer.len() as u64 {
            return Err("Build read exceeds the supplied buffer".into());
        }
        let ordinal = self.begin_attempt(
            FilesystemHostOperation::Read,
            root,
            &path,
            FilesystemGrantAccess::Read,
            Some(descriptor),
        )?;
        let bytes = self
            .snapshot(root)?
            .read(&path, cursor, count)
            .map_err(|error| format!("captured Build read failed: {error:?}"))?;
        let length = bytes.len();
        buffer[..length].copy_from_slice(bytes);
        if let Some(OpenFile {
            storage: FileStorage::Captured { cursor },
            ..
        }) = self.descriptors[slot].as_mut()
        {
            *cursor = cursor
                .checked_add(length as u64)
                .ok_or("Build read cursor overflowed")?;
        }
        Ok(self.complete_attempt(ordinal, length as i64, 0))
    }

    pub fn create(&mut self, path: &[u8], _mode: i32) -> Result<i32, String> {
        self.resolve(BUILD_OUTPUT_ROOT_IDENTITY, path)?;
        let ordinal = self.begin_attempt(
            FilesystemHostOperation::Create,
            BUILD_OUTPUT_ROOT_IDENTITY,
            path,
            FilesystemGrantAccess::Write,
            None,
        )?;
        let descriptor = i32::try_from(self.descriptors.len())
            .ok()
            .and_then(|value| value.checked_add(3))
            .ok_or("Build descriptor space exhausted")?;
        let handle = self
            .account
            .reserve_site(1, 1, path.len() as u64)
            .map_err(|error| error.to_string())?;
        self.descriptors
            .try_reserve(1)
            .map_err(|_| "cannot retain Build descriptor")?;
        let retained_path = copy_bytes(path)?;
        let physical = self
            .scope
            .build_dir
            .join(std::str::from_utf8(path).map_err(|_| "Build path is not UTF-8")?);
        let sponsor = self
            .scope
            .sponsor
            .as_ref()
            .ok_or("Output sponsor is absent")?;
        let bound = sponsor
            .bind_path(&physical)
            .map_err(|error| error.to_string())?;
        let prepared = sponsor
            .prepare_create_object_open(&bound, 0)
            .map_err(|error| error.to_string())?;
        let file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&physical)
        {
            Ok(file) => file,
            Err(error) => {
                prepared.abort();
                return Ok(
                    self.complete_attempt(ordinal, -1, error.raw_os_error().unwrap_or(5)) as i32,
                );
            }
        };
        let writer = prepared.commit().map_err(|error| error.to_string())?;
        self.descriptors.push(Some(OpenFile {
            root: BUILD_OUTPUT_ROOT_IDENTITY,
            path: retained_path,
            storage: FileStorage::Output {
                file,
                writer,
                extent: 0,
            },
            _handle: handle,
        }));
        self.effects.attempts[ordinal].logical_handle_output =
            Some(FilesystemLogicalHandleOutput::new(
                FilesystemLogicalHandleKind::Descriptor,
                logical_handle(descriptor)?,
                FilesystemLogicalHandleOutputSource::Created,
            ));
        self.effects.attempts[ordinal].result =
            FilesystemOperationResult::LogicalHandle(logical_handle(descriptor)?);
        self.incomplete_attempt = None;
        Ok(descriptor)
    }

    pub fn write(&mut self, descriptor: i32, bytes: &[u8]) -> Result<i64, String> {
        let slot = self.descriptor_slot(BUILD_OUTPUT_ROOT_IDENTITY, descriptor)?;
        let (path, _path_storage) = self.descriptor_path(slot)?;
        let ordinal = self.begin_attempt(
            FilesystemHostOperation::Write,
            BUILD_OUTPUT_ROOT_IDENTITY,
            &path,
            FilesystemGrantAccess::Write,
            Some(descriptor),
        )?;
        let Some(OpenFile {
            storage:
                FileStorage::Output {
                    file,
                    writer,
                    extent,
                },
            ..
        }) = self.descriptors[slot].as_mut()
        else {
            return Err("BuildOutput write requires an output descriptor".into());
        };
        let sponsor = self
            .scope
            .sponsor
            .as_ref()
            .ok_or("Output sponsor is absent")?;
        let prepared = sponsor
            .prepare_write(writer, *extent, bytes.len() as u64)
            .map_err(|error| error.to_string())?;
        let written = match file.write(bytes) {
            Ok(written) => written,
            Err(error) => {
                prepared.abort();
                return Ok(self.complete_attempt(ordinal, -1, error.raw_os_error().unwrap_or(5)));
            }
        };
        prepared
            .commit_written(written as u64)
            .map_err(|error| error.to_string())?;
        *extent = extent
            .checked_add(written as u64)
            .ok_or("Build output extent overflowed")?;
        Ok(self.complete_attempt(ordinal, written as i64, 0))
    }

    pub fn close(
        &mut self,
        root: FilesystemGrantRootIdentity,
        descriptor: i32,
    ) -> Result<i32, String> {
        let slot = self.descriptor_slot(root, descriptor)?;
        let (path, _path_storage) = self.descriptor_path(slot)?;
        let ordinal = self.begin_attempt(
            FilesystemHostOperation::Close,
            root,
            &path,
            FilesystemGrantAccess::Read,
            Some(descriptor),
        )?;
        let storage = self
            .account
            .reserve_site(0, 1, path.len() as u64)
            .map_err(|error| error.to_string())?;
        self.sealed
            .try_reserve(1)
            .map_err(|_| "cannot retain sealed output")?;
        self.effects
            .storage
            .try_reserve(1)
            .map_err(|_| "cannot retain sealed output reservation")?;
        if let Some(OpenFile {
            storage: FileStorage::Output { file, writer, .. },
            ..
        }) = self.descriptors[slot].as_mut()
        {
            file.flush().map_err(|error| error.to_string())?;
            self.scope
                .sponsor
                .as_ref()
                .ok_or("Output sponsor is absent")?
                .prepare_close(writer)
                .and_then(|prepared| prepared.commit())
                .map_err(|error| error.to_string())?;
            self.sealed.push((path, ordinal + 1));
            self.effects.storage.push(storage);
        }
        self.descriptors[slot] = None;
        self.effects.attempts[ordinal]
            .retired_logical_handles
            .push(logical_handle(descriptor)?);
        Ok(self.complete_attempt(ordinal, 0, 0) as i32)
    }

    fn descriptor_slot(
        &self,
        root: FilesystemGrantRootIdentity,
        descriptor: i32,
    ) -> Result<usize, String> {
        let slot = descriptor
            .checked_sub(3)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or("invalid Build descriptor")?;
        match self.descriptors.get(slot).and_then(Option::as_ref) {
            Some(file) if file.root == root => Ok(slot),
            _ => Err("Build descriptor is closed or belongs to another root".into()),
        }
    }

    fn descriptor_path(&self, slot: usize) -> Result<(Vec<u8>, EvaluationStorageLease), String> {
        let file = self
            .descriptors
            .get(slot)
            .and_then(Option::as_ref)
            .ok_or("closed Build descriptor")?;
        let storage = self
            .account
            .reserve_site(0, 1, file.path.len() as u64)
            .map_err(|error| error.to_string())?;
        Ok((copy_bytes(&file.path)?, storage))
    }

    pub fn require(&mut self, path: &[u8]) -> Result<usize, String> {
        self.resolve(BUILD_OUTPUT_ROOT_IDENTITY, path)?;
        if self.effects.obligations.len() >= filesystem_authority::MAX_BUILD_OUTPUT_OBLIGATIONS {
            return Err("Build required-output limit exhausted".into());
        }
        if self.effects.obligations.iter().any(|obligation| {
            obligation.relative_path == path
                || is_descendant(&obligation.relative_path, path)
                || is_descendant(path, &obligation.relative_path)
        }) {
            return Err("Build required-output names collide".into());
        }
        let storage = self
            .account
            .reserve_site(0, 1, path.len() as u64)
            .map_err(|error| error.to_string())?;
        let identity = self.effects.obligations.len();
        self.effects
            .obligations
            .try_reserve(1)
            .map_err(|_| "cannot retain output obligation")?;
        self.effects
            .storage
            .try_reserve(1)
            .map_err(|_| "cannot retain output obligation reservation")?;
        self.effects.obligations.push(BuildOutputObligation {
            root: BUILD_OUTPUT_ROOT_IDENTITY,
            relative_path: copy_bytes(path)?,
            issued_at: self.effects.attempts.len(),
            state: BuildOutputObligationState::Pending,
            post_completion_mutations: Vec::new(),
        });
        self.effects.storage.push(storage);
        Ok(identity)
    }

    pub fn can_complete(&self, obligation: usize, path: &[u8]) -> Result<bool, String> {
        let row = self
            .effects
            .obligations
            .get(obligation)
            .ok_or("unknown Build output obligation")?;
        if row.relative_path != path || row.state != BuildOutputObligationState::Pending {
            return Err("Build output obligation is consumed or names another output".into());
        }
        Ok(self.sealed.iter().any(|(sealed, _)| sealed == path))
    }

    pub fn complete_output(&mut self, obligation: usize, path: &[u8]) -> Result<usize, String> {
        if !self.can_complete(obligation, path)? {
            return Err("Build output is not sealed".into());
        }
        let receipt = self.effects.receipts.len();
        let completed_at = self.effects.attempts.len();
        let sealed_at = self
            .sealed
            .iter()
            .find(|(sealed, _)| sealed == path)
            .map(|(_, ordinal)| *ordinal)
            .ok_or("Build output is not sealed")?;
        let storage = self
            .account
            .reserve_site(0, 1, path.len() as u64)
            .map_err(|error| error.to_string())?;
        self.effects
            .receipts
            .try_reserve(1)
            .map_err(|_| "cannot retain output receipt")?;
        self.effects
            .storage
            .try_reserve(1)
            .map_err(|_| "cannot retain output receipt reservation")?;
        self.effects.receipts.push(BuildOutputReceipt {
            obligation,
            root: BUILD_OUTPUT_ROOT_IDENTITY,
            relative_path: copy_bytes(path)?,
            sealed_at,
            completed_at,
        });
        self.effects.storage.push(storage);
        self.effects.obligations[obligation].state = BuildOutputObligationState::Completed {
            completed_at,
            receipt,
        };
        Ok(receipt)
    }

    pub fn fail_output(
        &mut self,
        obligation: usize,
        diagnostic: &[u8],
        machine_name: &str,
    ) -> Result<(), String> {
        let storage = self
            .account
            .reserve_site(0, 1, diagnostic.len() as u64)
            .map_err(|error| error.to_string())?;
        self.effects
            .storage
            .try_reserve(1)
            .map_err(|_| "cannot retain output failure reservation")?;
        let row = self
            .effects
            .obligations
            .get_mut(obligation)
            .ok_or("unknown Build output obligation")?;
        if row.state != BuildOutputObligationState::Pending {
            return Err("Build output obligation was already consumed".into());
        }
        row.state = BuildOutputObligationState::Failed {
            failed_at: self.effects.attempts.len(),
            diagnostic: copy_bytes(diagnostic)?,
        };
        self.effects.storage.push(storage);
        self.refused = Some(format!(
            "required output `{}` of `{machine_name}` failed: {}",
            String::from_utf8_lossy(&row.relative_path),
            String::from_utf8_lossy(diagnostic)
        ));
        Err(self.refused.clone().unwrap_or_default())
    }

    pub fn include_source(&mut self, path: &[u8]) -> Result<(), String> {
        if !self.sealed.iter().any(|(sealed, _)| sealed == path) {
            return Err("staging entry is not sealed: generated source still has a live writer or was never created".into());
        }
        let storage = self
            .account
            .reserve_site(0, 1, path.len() as u64)
            .map_err(|error| error.to_string())?;
        self.effects
            .included
            .try_reserve(1)
            .map_err(|_| "cannot retain generated-source handoff")?;
        self.effects
            .storage
            .try_reserve(1)
            .map_err(|_| "cannot retain generated-source reservation")?;
        self.effects.included.push(BuildIncludedSourceHandoff {
            relative_path: copy_bytes(path)?,
            filesystem_attempt_ordinal: self.effects.attempts.len() as u64,
        });
        self.effects.storage.push(storage);
        Ok(())
    }

    pub fn finish(self) -> Result<BuildFilesystemEffects, String> {
        if let Some(reason) = self.refused {
            return Err(reason);
        }
        if self.incomplete_attempt.is_some() {
            return Err("Build filesystem operation did not complete".into());
        }
        let mut rows = 0u64;
        let mut bytes = 0u64;
        for attempt in &self.effects.attempts {
            rows = rows
                .checked_add(1)
                .ok_or("filesystem result row count overflowed")?;
            for path in &attempt.authorized_paths {
                bytes = bytes
                    .checked_add(path.relative_path.len() as u64)
                    .ok_or("filesystem result payload overflowed")?;
            }
        }
        for path in self
            .effects
            .included
            .iter()
            .map(|row| row.relative_path.as_slice())
            .chain(
                self.effects
                    .obligations
                    .iter()
                    .map(|row| row.relative_path.as_slice()),
            )
            .chain(
                self.effects
                    .receipts
                    .iter()
                    .map(|row| row.relative_path.as_slice()),
            )
        {
            rows = rows
                .checked_add(1)
                .ok_or("filesystem result row count overflowed")?;
            bytes = bytes
                .checked_add(path.len() as u64)
                .ok_or("filesystem result payload overflowed")?;
        }
        self.account
            .charge_result_custody(rows, bytes)
            .map_err(|error| error.to_string())?;
        Ok(self.effects)
    }
}

fn logical_handle(descriptor: i32) -> Result<FilesystemLogicalHandleIdentity, String> {
    let identity = u64::try_from(descriptor)
        .map_err(|_| "negative Build descriptor")?
        .checked_add(1)
        .ok_or("Build descriptor identity overflowed")?;
    FilesystemLogicalHandleIdentity::new(identity)
        .ok_or_else(|| "zero Build descriptor identity".into())
}

fn is_descendant(path: &[u8], parent: &[u8]) -> bool {
    path.strip_prefix(parent)
        .is_some_and(|tail| tail.starts_with(b"/"))
}

fn copy_bytes(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut retained = Vec::new();
    retained
        .try_reserve_exact(bytes.len())
        .map_err(|_| "filesystem byte allocation was refused")?;
    retained.extend_from_slice(bytes);
    Ok(retained)
}

#[cfg(test)]
mod tests {
    use super::*;
    use build_output::{CapturedBuildSourceInput, CapturedSourceEntry};
    use filesystem_authority::{
        CanonicalFilesystemMetadataIndex, CanonicalFilesystemMetadataRow,
        CanonicalFilesystemMetadataRowKind as Kind,
    };

    fn scope(root: &std::path::Path, name: &str) -> BuildMachineFilesystemScope {
        let source_root = root.join("source");
        std::fs::create_dir_all(&source_root).unwrap();
        let sponsor = filesystem_authority::FilesystemSponsor::create_private(
            &root.join(name),
            filesystem_authority::FilesystemSponsorLimits::COMPILER_DEFAULT,
        )
        .unwrap();
        let output = sponsor.session_root().unwrap().join("output");
        let metadata = CanonicalFilesystemMetadataIndex::version_1(
            [42; 32],
            [
                CanonicalFilesystemMetadataRow::new(b"", Kind::Directory),
                CanonicalFilesystemMetadataRow::new(
                    b"asset",
                    Kind::File {
                        executable: false,
                        logical_byte_length: 5,
                    },
                ),
            ],
        )
        .unwrap();
        let input = CapturedBuildSourceInput::from_capture_rows(
            metadata,
            [(
                b"asset".to_vec(),
                CapturedSourceEntry::file(&b"hello"[..], false),
            )],
        )
        .unwrap();
        let scope = BuildMachineFilesystemScope::for_root(
            &source_root.join("main.omg"),
            output,
            Some(sponsor),
        )
        .with_captured_source_input(input)
        .unwrap();
        scope.establish_filesystem_access().unwrap();
        scope
    }

    fn account() -> EvaluationInvocationAccount {
        let limits = evaluation_accounting::BuildEvaluationSponsorLimits::new(
            1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000,
        )
        .unwrap();
        let sponsor = evaluation_accounting::BuildEvaluationSponsor::new(limits);
        sponsor
            .begin_invocation(sponsor.measurement(), 1_000_000)
            .unwrap()
    }

    #[test]
    fn captured_read_and_output_receipt_preserve_exact_custody() {
        let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
            "build-filesystem-custody",
            false,
        )
        .unwrap();
        let scope = scope(temporary.path(), "staging");
        let mut runtime = scope.begin_filesystem_runtime(account()).unwrap();
        let reader = runtime
            .open(BUILD_SOURCE_ROOT_IDENTITY, b"asset", 0)
            .unwrap();
        let mut bytes = [0; 5];
        assert_eq!(
            runtime
                .read(BUILD_SOURCE_ROOT_IDENTITY, reader, &mut bytes, 5)
                .unwrap(),
            5
        );
        assert_eq!(&bytes, b"hello");
        runtime.close(BUILD_SOURCE_ROOT_IDENTITY, reader).unwrap();
        let obligation = runtime.require(b"generated.omg").unwrap();
        assert!(!runtime.can_complete(obligation, b"generated.omg").unwrap());
        let writer = runtime.create(b"generated.omg", 0).unwrap();
        runtime.write(writer, &bytes).unwrap();
        assert!(!runtime.can_complete(obligation, b"generated.omg").unwrap());
        runtime.close(BUILD_OUTPUT_ROOT_IDENTITY, writer).unwrap();
        assert!(runtime.complete_output(obligation, b"other.omg").is_err());
        runtime
            .complete_output(obligation, b"generated.omg")
            .unwrap();
        assert!(
            runtime
                .complete_output(obligation, b"generated.omg")
                .is_err()
        );
        runtime.include_source(b"generated.omg").unwrap();
        let effects = runtime.finish().unwrap();
        let mut observations = crate::CapturedBuildObservations::new(&scope);
        observations.retain_filesystem_effects(effects).unwrap();
        let summary = observations.settle(true, "build").unwrap();
        assert_eq!(summary.required_output_settlements().len(), 1);
        assert_eq!(
            summary.select_generated_sources("build").unwrap()[0].bytes(),
            b"hello"
        );
    }

    #[test]
    fn denied_source_write_is_recorded_and_effects_cannot_change_scope() {
        let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
            "build-filesystem-scope",
            false,
        )
        .unwrap();
        let first = scope(temporary.path(), "first");
        let second = scope(temporary.path(), "second");
        let mut runtime = first.begin_filesystem_runtime(account()).unwrap();
        assert_eq!(
            runtime
                .open(BUILD_SOURCE_ROOT_IDENTITY, b"asset", 1)
                .unwrap(),
            -1
        );
        let effects = runtime.finish().unwrap();
        assert!(effects.attempts[0].authorized_paths.is_empty());
        assert_eq!(effects.attempts[0].grant_refusals.len(), 1);
        assert_eq!(effects.attempts[0].post_error, 13);
        assert!(
            crate::CapturedBuildObservations::new(&second)
                .retain_filesystem_effects(effects)
                .is_err()
        );
    }
}
