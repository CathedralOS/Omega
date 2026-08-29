use super::model::{
    CanonicalPackageReconstructionEntry, CanonicalPackageReconstructionQuestion,
    CanonicalPackageReconstructionQuestionError, CanonicalPackageReconstructionQuestionFingerprint,
    CanonicalPackageReconstructionQuestionLimits,
};
use super::{
    PACKAGE_RECONSTRUCTION_QUESTION_ENCODING_VERSION, RECONSTRUCTION_QUESTION_FINGERPRINT_DOMAIN,
    RECONSTRUCTION_QUESTION_MAGIC,
};
use crate::CanonicalSourceClosureSubject;
use omega_package_review::{
    decode_ordinary_package_obligation_ledger, encode_ordinary_package_obligation_ledger,
};
use sha2::{Digest, Sha256};

impl CanonicalPackageReconstructionQuestion {
    /// Strictly recover canonical association bytes.
    ///
    /// The recovered value remains an inert question until independently
    /// compared with current resolver custody and newly compiled reviews.
    pub fn recover(
        bytes: &[u8],
        limits: CanonicalPackageReconstructionQuestionLimits,
    ) -> Result<Self, CanonicalPackageReconstructionQuestionError> {
        let limits = limits.compiler_bounded();
        if bytes.len() > limits.maximum_record_bytes {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "package reconstruction question exceeds its record-byte ceiling",
            ));
        }
        let mut decoder = Decoder::new(bytes);
        decoder.expect_fixed(RECONSTRUCTION_QUESTION_MAGIC)?;
        if decoder.u16()? != PACKAGE_RECONSTRUCTION_QUESTION_ENCODING_VERSION {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "unsupported package reconstruction question version",
            ));
        }
        let source_bytes = decoder.bytes(limits.source_closure.maximum_record_bytes)?;
        let source_closure =
            CanonicalSourceClosureSubject::recover(source_bytes, limits.source_closure).map_err(
                |_| {
                    CanonicalPackageReconstructionQuestionError::new(
                        "package reconstruction question contains an invalid source subject",
                    )
                },
            )?;
        let entry_count = decoder.count(limits.maximum_packages)?;
        if entry_count != source_closure.packages().len() {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "source closure and obligation ledger count are not bijective",
            ));
        }

        let mut entries = Vec::new();
        entries.try_reserve_exact(entry_count).map_err(|_| {
            CanonicalPackageReconstructionQuestionError::new(
                "package reconstruction entry allocation failed",
            )
        })?;
        let mut total_ledger_bytes = 0usize;
        for selected in source_closure.packages() {
            let ledger_bytes = decoder.bytes(limits.maximum_ledger_bytes)?;
            total_ledger_bytes = total_ledger_bytes
                .checked_add(ledger_bytes.len())
                .ok_or_else(|| {
                    CanonicalPackageReconstructionQuestionError::new(
                        "package reconstruction ledger-byte accounting overflowed",
                    )
                })?;
            if total_ledger_bytes > limits.maximum_total_ledger_bytes {
                return Err(CanonicalPackageReconstructionQuestionError::new(
                    "package reconstruction question exceeds its total ledger-byte ceiling",
                ));
            }
            let obligation_ledger = decode_ordinary_package_obligation_ledger(ledger_bytes)
                .map_err(|_| {
                    CanonicalPackageReconstructionQuestionError::new(
                        "package reconstruction question contains an invalid obligation ledger",
                    )
                })?;
            entries.push(CanonicalPackageReconstructionEntry {
                package: selected.key().clone(),
                obligation_ledger,
            });
        }
        decoder.finish()?;

        let recovered = Self::finish(source_closure, entries, limits)?;
        if recovered.canonical_bytes != bytes {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "package reconstruction question is not canonically encoded",
            ));
        }
        Ok(recovered)
    }
}

pub(super) fn encode_question(
    source_closure: &CanonicalSourceClosureSubject,
    entries: &[CanonicalPackageReconstructionEntry],
    limits: CanonicalPackageReconstructionQuestionLimits,
) -> Result<Vec<u8>, CanonicalPackageReconstructionQuestionError> {
    let mut encoder = Encoder::bounded(limits.maximum_record_bytes);
    encoder.fixed(RECONSTRUCTION_QUESTION_MAGIC)?;
    encoder.u16(PACKAGE_RECONSTRUCTION_QUESTION_ENCODING_VERSION)?;
    encoder.bytes(source_closure.canonical_bytes())?;
    encoder.count(entries.len())?;
    for entry in entries {
        let ledger_bytes = encode_ordinary_package_obligation_ledger(&entry.obligation_ledger)
            .map_err(|_| {
                CanonicalPackageReconstructionQuestionError::new(
                    "package reconstruction question contains an invalid obligation ledger",
                )
            })?;
        encoder.bytes(&ledger_bytes)?;
    }
    encoder.finish()
}

pub(super) fn fingerprint(bytes: &[u8]) -> CanonicalPackageReconstructionQuestionFingerprint {
    let mut digest = Sha256::new();
    digest.update(RECONSTRUCTION_QUESTION_FINGERPRINT_DOMAIN);
    digest.update(
        u64::try_from(bytes.len())
            .expect("bounded reconstruction question length fits u64")
            .to_le_bytes(),
    );
    digest.update(bytes);
    CanonicalPackageReconstructionQuestionFingerprint(digest.finalize().into())
}

struct Encoder {
    output: Vec<u8>,
    maximum_bytes: usize,
}

impl Encoder {
    fn bounded(maximum_bytes: usize) -> Self {
        Self {
            output: Vec::new(),
            maximum_bytes,
        }
    }

    fn reserve(
        &mut self,
        additional: usize,
    ) -> Result<(), CanonicalPackageReconstructionQuestionError> {
        let required = self.output.len().checked_add(additional).ok_or_else(|| {
            CanonicalPackageReconstructionQuestionError::new(
                "package reconstruction encoding length overflowed",
            )
        })?;
        if required > self.maximum_bytes {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "package reconstruction question exceeds its record-byte ceiling",
            ));
        }
        self.output.try_reserve_exact(additional).map_err(|_| {
            CanonicalPackageReconstructionQuestionError::new(
                "package reconstruction encoding allocation failed",
            )
        })
    }

    fn fixed(&mut self, bytes: &[u8]) -> Result<(), CanonicalPackageReconstructionQuestionError> {
        self.reserve(bytes.len())?;
        self.output.extend_from_slice(bytes);
        Ok(())
    }

    fn u16(&mut self, value: u16) -> Result<(), CanonicalPackageReconstructionQuestionError> {
        self.fixed(&value.to_le_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<(), CanonicalPackageReconstructionQuestionError> {
        self.fixed(&value.to_le_bytes())
    }

    fn count(&mut self, value: usize) -> Result<(), CanonicalPackageReconstructionQuestionError> {
        let value = u32::try_from(value).map_err(|_| {
            CanonicalPackageReconstructionQuestionError::new(
                "package reconstruction sequence count exceeds u32",
            )
        })?;
        self.u32(value)
    }

    fn bytes(&mut self, bytes: &[u8]) -> Result<(), CanonicalPackageReconstructionQuestionError> {
        self.count(bytes.len())?;
        self.fixed(bytes)
    }

    fn finish(self) -> Result<Vec<u8>, CanonicalPackageReconstructionQuestionError> {
        Ok(self.output)
    }
}

struct Decoder<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
}

impl<'bytes> Decoder<'bytes> {
    const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(
        &mut self,
        count: usize,
    ) -> Result<&'bytes [u8], CanonicalPackageReconstructionQuestionError> {
        let end = self.offset.checked_add(count).ok_or_else(|| {
            CanonicalPackageReconstructionQuestionError::new(
                "package reconstruction decoding offset overflowed",
            )
        })?;
        let taken = self.bytes.get(self.offset..end).ok_or_else(|| {
            CanonicalPackageReconstructionQuestionError::new(
                "truncated package reconstruction question",
            )
        })?;
        self.offset = end;
        Ok(taken)
    }

    fn expect_fixed(
        &mut self,
        expected: &[u8],
    ) -> Result<(), CanonicalPackageReconstructionQuestionError> {
        if self.take(expected.len())? == expected {
            Ok(())
        } else {
            Err(CanonicalPackageReconstructionQuestionError::new(
                "invalid package reconstruction question magic",
            ))
        }
    }

    fn u16(&mut self) -> Result<u16, CanonicalPackageReconstructionQuestionError> {
        Ok(u16::from_le_bytes(
            self.take(2)?
                .try_into()
                .expect("fixed two-byte decoder slice"),
        ))
    }

    fn u32(&mut self) -> Result<u32, CanonicalPackageReconstructionQuestionError> {
        Ok(u32::from_le_bytes(
            self.take(4)?
                .try_into()
                .expect("fixed four-byte decoder slice"),
        ))
    }

    fn count(
        &mut self,
        maximum: usize,
    ) -> Result<usize, CanonicalPackageReconstructionQuestionError> {
        let value = usize::try_from(self.u32()?).map_err(|_| {
            CanonicalPackageReconstructionQuestionError::new(
                "package reconstruction count exceeds platform range",
            )
        })?;
        if value > maximum {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "package reconstruction count exceeds its ceiling",
            ));
        }
        Ok(value)
    }

    fn bytes(
        &mut self,
        maximum: usize,
    ) -> Result<&'bytes [u8], CanonicalPackageReconstructionQuestionError> {
        let count = self.count(maximum)?;
        self.take(count)
    }

    fn finish(self) -> Result<(), CanonicalPackageReconstructionQuestionError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(CanonicalPackageReconstructionQuestionError::new(
                "package reconstruction question contains trailing bytes",
            ))
        }
    }
}
