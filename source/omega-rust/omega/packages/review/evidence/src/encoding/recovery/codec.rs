use super::model::PackageReviewCanonicalRowRecoveryError;

pub(super) fn clone_bytes(
    bytes: &[u8],
    allocation_error: &'static str,
) -> Result<Vec<u8>, PackageReviewCanonicalRowRecoveryError> {
    let mut output = Vec::new();
    output
        .try_reserve(bytes.len())
        .map_err(|_| PackageReviewCanonicalRowRecoveryError::new(allocation_error))?;
    output.extend_from_slice(bytes);
    Ok(output)
}

pub(super) fn clone_string(
    value: &str,
    allocation_error: &'static str,
) -> Result<String, PackageReviewCanonicalRowRecoveryError> {
    let mut output = String::new();
    output
        .try_reserve(value.len())
        .map_err(|_| PackageReviewCanonicalRowRecoveryError::new(allocation_error))?;
    output.push_str(value);
    Ok(output)
}

pub(super) struct RecoveryEncoder {
    output: Vec<u8>,
    maximum_bytes: usize,
    exceeded: bool,
}

impl RecoveryEncoder {
    pub(super) fn bounded(maximum_bytes: usize) -> Self {
        Self {
            output: Vec::new(),
            maximum_bytes,
            exceeded: false,
        }
    }

    fn append(&mut self, bytes: &[u8]) {
        if self.exceeded {
            return;
        }
        let Some(required) = self.output.len().checked_add(bytes.len()) else {
            self.exceeded = true;
            return;
        };
        if required > self.maximum_bytes || self.output.try_reserve(bytes.len()).is_err() {
            self.exceeded = true;
            return;
        }
        self.output.extend_from_slice(bytes);
    }

    pub(super) fn finish(self) -> Result<Vec<u8>, PackageReviewCanonicalRowRecoveryError> {
        if self.exceeded {
            Err(PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery envelope exceeds its byte ceiling",
            ))
        } else {
            Ok(self.output)
        }
    }

    pub(super) fn fixed_bytes(&mut self, bytes: &[u8]) {
        self.append(bytes);
    }

    pub(super) fn byte(&mut self, value: u8) {
        self.append(&[value]);
    }

    pub(super) fn u16(&mut self, value: u16) {
        self.append(&value.to_le_bytes());
    }

    pub(super) fn u64(&mut self, value: u64) {
        self.append(&value.to_le_bytes());
    }

    pub(super) fn usize(
        &mut self,
        value: usize,
    ) -> Result<(), PackageReviewCanonicalRowRecoveryError> {
        self.u64(u64::try_from(value).map_err(|_| {
            PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery value exceeds the portable encoding range",
            )
        })?);
        Ok(())
    }

    pub(super) fn bytes(
        &mut self,
        bytes: &[u8],
    ) -> Result<(), PackageReviewCanonicalRowRecoveryError> {
        self.usize(bytes.len())?;
        self.append(bytes);
        Ok(())
    }

    pub(super) fn string(
        &mut self,
        value: &str,
    ) -> Result<(), PackageReviewCanonicalRowRecoveryError> {
        self.bytes(value.as_bytes())
    }
}

pub(super) struct RecoveryDecoder<'bytes> {
    bytes: &'bytes [u8],
    position: usize,
}

impl<'bytes> RecoveryDecoder<'bytes> {
    pub(super) const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn take(
        &mut self,
        count: usize,
    ) -> Result<&'bytes [u8], PackageReviewCanonicalRowRecoveryError> {
        let end = self.position.checked_add(count).ok_or_else(|| {
            PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery length frame overflow",
            )
        })?;
        let value = self.bytes.get(self.position..end).ok_or_else(|| {
            PackageReviewCanonicalRowRecoveryError::new("canonical-row recovery input is truncated")
        })?;
        self.position = end;
        Ok(value)
    }

    pub(super) fn fixed_bytes(
        &mut self,
        expected: &[u8],
    ) -> Result<(), PackageReviewCanonicalRowRecoveryError> {
        if self.take(expected.len())? != expected {
            return Err(PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery input has invalid framing magic",
            ));
        }
        Ok(())
    }

    pub(super) fn byte(&mut self) -> Result<u8, PackageReviewCanonicalRowRecoveryError> {
        Ok(self.take(1)?[0])
    }

    pub(super) fn u16(&mut self) -> Result<u16, PackageReviewCanonicalRowRecoveryError> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("two-byte decoder slice"),
        ))
    }

    pub(super) fn u64(&mut self) -> Result<u64, PackageReviewCanonicalRowRecoveryError> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("eight-byte decoder slice"),
        ))
    }

    pub(super) fn usize(&mut self) -> Result<usize, PackageReviewCanonicalRowRecoveryError> {
        usize::try_from(self.u64()?).map_err(|_| {
            PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery length exceeds the host range",
            )
        })
    }

    pub(super) fn bytes(
        &mut self,
        maximum_bytes: usize,
    ) -> Result<&'bytes [u8], PackageReviewCanonicalRowRecoveryError> {
        let count = self.usize()?;
        if count > maximum_bytes {
            return Err(PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery length frame exceeds its field ceiling",
            ));
        }
        self.take(count)
    }

    pub(super) fn string(
        &mut self,
        maximum_bytes: usize,
    ) -> Result<&'bytes str, PackageReviewCanonicalRowRecoveryError> {
        std::str::from_utf8(self.bytes(maximum_bytes)?).map_err(|_| {
            PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery string is not valid UTF-8",
            )
        })
    }

    pub(super) fn array_32(&mut self) -> Result<[u8; 32], PackageReviewCanonicalRowRecoveryError> {
        Ok(self
            .take(32)?
            .try_into()
            .expect("thirty-two-byte decoder slice"))
    }

    pub(super) fn finish(self) -> Result<(), PackageReviewCanonicalRowRecoveryError> {
        if self.position != self.bytes.len() {
            return Err(PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery input contains trailing bytes",
            ));
        }
        Ok(())
    }
}
