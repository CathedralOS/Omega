//! The byte encoder and decoder.

use crate::replay_record::{BuildFilesystemReplayRecordError, BuildFilesystemReplayRecordLimits};
use crate::{
    BuildFilesystemGrantAccess, BuildFilesystemGrantRefusalReason,
    BuildFilesystemLogicalHandleKind, BuildFilesystemMetadataObservationKind,
    BuildFilesystemObservedByteRegionKind, BuildFilesystemProvider,
    BuildFilesystemReturnedPathCompleteness, BuildFilesystemReturnedPathKind, BuildFilesystemRoot,
};

pub(crate) struct Encoder {
    bytes: Vec<u8>,
    maximum: usize,
    exceeded: bool,
}

impl Encoder {
    pub(crate) fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
            exceeded: false,
        }
    }
    fn append(&mut self, bytes: &[u8]) {
        if self.exceeded
            || self
                .bytes
                .len()
                .checked_add(bytes.len())
                .is_none_or(|length| length > self.maximum)
        {
            self.exceeded = true;
            return;
        }
        if self.bytes.try_reserve(bytes.len()).is_err() {
            self.exceeded = true;
            return;
        }
        self.bytes.extend_from_slice(bytes);
    }
    pub(crate) fn fixed(&mut self, value: &[u8]) {
        self.append(value);
    }
    pub(crate) fn byte(&mut self, value: u8) {
        self.append(&[value]);
    }
    pub(crate) fn u16(&mut self, value: u16) {
        self.append(&value.to_le_bytes());
    }
    pub(crate) fn u32(&mut self, value: u32) {
        self.append(&value.to_le_bytes());
    }
    pub(crate) fn i32(&mut self, value: i32) {
        self.append(&value.to_le_bytes());
    }
    pub(crate) fn u64(&mut self, value: u64) {
        self.append(&value.to_le_bytes());
    }
    pub(crate) fn i64(&mut self, value: i64) {
        self.append(&value.to_le_bytes());
    }
    pub(crate) fn count(&mut self, value: usize) -> Result<(), BuildFilesystemReplayRecordError> {
        self.u64(
            u64::try_from(value)
                .map_err(|_| BuildFilesystemReplayRecordError::new("replay count exceeds u64"))?,
        );
        Ok(())
    }
    pub(crate) fn bytes(&mut self, value: &[u8]) -> Result<(), BuildFilesystemReplayRecordError> {
        self.count(value.len())?;
        self.append(value);
        Ok(())
    }
    pub(crate) fn finish(self) -> Result<Vec<u8>, BuildFilesystemReplayRecordError> {
        if self.exceeded {
            Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay record exceeds its byte ceiling",
            ))
        } else {
            Ok(self.bytes)
        }
    }
}

pub(crate) struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
    limits: BuildFilesystemReplayRecordLimits,
}

impl<'a> Decoder<'a> {
    pub(crate) const fn new(bytes: &'a [u8], limits: BuildFilesystemReplayRecordLimits) -> Self {
        Self {
            bytes,
            offset: 0,
            limits,
        }
    }
    fn take(&mut self, length: usize) -> Result<&'a [u8], BuildFilesystemReplayRecordError> {
        let end = self.offset.checked_add(length).ok_or_else(|| {
            BuildFilesystemReplayRecordError::new("filesystem replay record length overflow")
        })?;
        let value = self.bytes.get(self.offset..end).ok_or_else(|| {
            BuildFilesystemReplayRecordError::new("truncated filesystem replay record")
        })?;
        self.offset = end;
        Ok(value)
    }
    pub(crate) fn fixed(
        &mut self,
        expected: &[u8],
    ) -> Result<(), BuildFilesystemReplayRecordError> {
        if self.take(expected.len())? == expected {
            Ok(())
        } else {
            Err(BuildFilesystemReplayRecordError::new(
                "invalid filesystem replay record magic",
            ))
        }
    }
    pub(crate) fn byte(&mut self) -> Result<u8, BuildFilesystemReplayRecordError> {
        Ok(self.take(1)?[0])
    }
    pub(crate) fn tag(
        &mut self,
        maximum: u8,
        message: &'static str,
    ) -> Result<u8, BuildFilesystemReplayRecordError> {
        let value = self.byte()?;
        if value <= maximum {
            Ok(value)
        } else {
            Err(BuildFilesystemReplayRecordError::new(message))
        }
    }
    pub(crate) fn u16(&mut self) -> Result<u16, BuildFilesystemReplayRecordError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub(crate) fn u32(&mut self) -> Result<u32, BuildFilesystemReplayRecordError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub(crate) fn array_32(&mut self) -> Result<[u8; 32], BuildFilesystemReplayRecordError> {
        Ok(self.take(32)?.try_into().unwrap())
    }
    pub(crate) fn i32(&mut self) -> Result<i32, BuildFilesystemReplayRecordError> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub(crate) fn u64(&mut self) -> Result<u64, BuildFilesystemReplayRecordError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub(crate) fn i64(&mut self) -> Result<i64, BuildFilesystemReplayRecordError> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub(crate) fn nonzero_u64(&mut self) -> Result<u64, BuildFilesystemReplayRecordError> {
        let value = self.u64()?;
        if value == 0 {
            Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay record contains a zero handle identity",
            ))
        } else {
            Ok(value)
        }
    }
    pub(crate) fn count(&mut self) -> Result<usize, BuildFilesystemReplayRecordError> {
        let value = usize::try_from(self.u64()?).map_err(|_| {
            BuildFilesystemReplayRecordError::new("filesystem replay count exceeds this host")
        })?;
        if value > self.limits.maximum_items_per_lane {
            Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay lane exceeds its item ceiling",
            ))
        } else {
            Ok(value)
        }
    }
    pub(crate) fn bytes(&mut self) -> Result<&'a [u8], BuildFilesystemReplayRecordError> {
        let length = usize::try_from(self.u64()?).map_err(|_| {
            BuildFilesystemReplayRecordError::new("filesystem replay byte length exceeds this host")
        })?;
        if length > self.limits.maximum_bytes {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay field exceeds its byte ceiling",
            ));
        }
        self.take(length)
    }
    pub(crate) fn finish(self) -> Result<(), BuildFilesystemReplayRecordError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay record has trailing bytes",
            ))
        }
    }
}

pub(crate) const fn provider_tag(value: BuildFilesystemProvider) -> u8 {
    match value {
        BuildFilesystemProvider::Virtual => 0,
        BuildFilesystemProvider::RealUnscoped => 1,
        BuildFilesystemProvider::RealScoped => 2,
    }
}

pub(crate) const fn access_tag(value: BuildFilesystemGrantAccess) -> u8 {
    match value {
        BuildFilesystemGrantAccess::Read => 0,
        BuildFilesystemGrantAccess::Write => 1,
    }
}

pub(crate) const fn root_tag(value: BuildFilesystemRoot) -> u8 {
    match value {
        BuildFilesystemRoot::Source => 0,
        BuildFilesystemRoot::Output => 1,
    }
}

pub(crate) const fn handle_kind_tag(value: BuildFilesystemLogicalHandleKind) -> u8 {
    match value {
        BuildFilesystemLogicalHandleKind::Descriptor => 0,
        BuildFilesystemLogicalHandleKind::Native => 1,
        BuildFilesystemLogicalHandleKind::Find => 2,
    }
}

pub(crate) const fn refusal_reason_tag(value: BuildFilesystemGrantRefusalReason) -> u8 {
    match value {
        BuildFilesystemGrantRefusalReason::Unresolvable => 0,
        BuildFilesystemGrantRefusalReason::OutsideGrantedRoots => 1,
        BuildFilesystemGrantRefusalReason::UnrepresentableRootedPath => 2,
        BuildFilesystemGrantRefusalReason::ObservationEvidenceLimitExceeded => 3,
    }
}

pub(crate) const fn returned_path_kind_tag(value: BuildFilesystemReturnedPathKind) -> u8 {
    match value {
        BuildFilesystemReturnedPathKind::ReadLinkPayload => 0,
        BuildFilesystemReturnedPathKind::CanonicalPath => 1,
        BuildFilesystemReturnedPathKind::FinalPath => 2,
    }
}

pub(crate) const fn returned_path_completeness_tag(
    value: BuildFilesystemReturnedPathCompleteness,
) -> u8 {
    match value {
        BuildFilesystemReturnedPathCompleteness::Complete => 0,
        BuildFilesystemReturnedPathCompleteness::LimitReached => 1,
    }
}

pub(crate) const fn observed_region_kind_tag(value: BuildFilesystemObservedByteRegionKind) -> u8 {
    match value {
        BuildFilesystemObservedByteRegionKind::SequentialFileRead => 0,
        BuildFilesystemObservedByteRegionKind::PositionedFileRead => 1,
        BuildFilesystemObservedByteRegionKind::DirectoryRecords => 2,
        BuildFilesystemObservedByteRegionKind::FindEntry => 3,
    }
}

pub(crate) const fn metadata_kind_tag(value: BuildFilesystemMetadataObservationKind) -> u8 {
    match value {
        BuildFilesystemMetadataObservationKind::FollowedPath => 0,
        BuildFilesystemMetadataObservationKind::OpenDescriptor => 1,
        BuildFilesystemMetadataObservationKind::UnfollowedFinalPath => 2,
    }
}
