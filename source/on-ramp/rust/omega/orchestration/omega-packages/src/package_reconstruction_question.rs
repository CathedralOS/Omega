use crate::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
    CompilerIssuedPackageReviewSet, PackageKey, ResolvedPackageSourceClosure,
    package_compilation_inputs_for,
};
use omega_compiler::{
    OrdinaryPackageObligationLedger, decode_ordinary_package_obligation_ledger,
    encode_ordinary_package_obligation_ledger,
};
use psi_core::PackageKeyIdentity;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const RECONSTRUCTION_QUESTION_MAGIC: &[u8] = b"OMEGA-PACKAGE-RECONSTRUCTION-QUESTION\0";
pub const PACKAGE_RECONSTRUCTION_QUESTION_ENCODING_VERSION: u16 = 1;
const RECONSTRUCTION_QUESTION_FINGERPRINT_DOMAIN: &[u8] =
    b"OMEGA-PACKAGE-RECONSTRUCTION-QUESTION-FINGERPRINT\0";
const ABSOLUTE_RECORD_BYTE_LIMIT: usize = 128 * 1024 * 1024;
const ABSOLUTE_PACKAGE_LIMIT: usize = 16 * 1024;
const ABSOLUTE_LEDGER_BYTE_LIMIT: usize = 32 * 1024 * 1024;
const ABSOLUTE_TOTAL_LEDGER_BYTE_LIMIT: usize = 64 * 1024 * 1024;

/// Resource ceilings for one source-to-obligation reconstruction question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalPackageReconstructionQuestionLimits {
    pub maximum_record_bytes: usize,
    pub maximum_packages: usize,
    pub maximum_ledger_bytes: usize,
    pub maximum_total_ledger_bytes: usize,
    pub source_closure: CanonicalSourceClosureSubjectLimits,
}

impl Default for CanonicalPackageReconstructionQuestionLimits {
    fn default() -> Self {
        Self {
            maximum_record_bytes: ABSOLUTE_RECORD_BYTE_LIMIT,
            maximum_packages: 1024,
            maximum_ledger_bytes: ABSOLUTE_LEDGER_BYTE_LIMIT,
            maximum_total_ledger_bytes: ABSOLUTE_TOTAL_LEDGER_BYTE_LIMIT,
            source_closure: CanonicalSourceClosureSubjectLimits::default(),
        }
    }
}

impl CanonicalPackageReconstructionQuestionLimits {
    fn compiler_bounded(self) -> Self {
        Self {
            maximum_record_bytes: self.maximum_record_bytes.min(ABSOLUTE_RECORD_BYTE_LIMIT),
            maximum_packages: self.maximum_packages.min(ABSOLUTE_PACKAGE_LIMIT),
            maximum_ledger_bytes: self.maximum_ledger_bytes.min(ABSOLUTE_LEDGER_BYTE_LIMIT),
            maximum_total_ledger_bytes: self
                .maximum_total_ledger_bytes
                .min(ABSOLUTE_TOTAL_LEDGER_BYTE_LIMIT),
            source_closure: self.source_closure,
        }
    }
}

/// A closed failure while associating or strictly recovering a reconstruction
/// question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalPackageReconstructionQuestionError {
    message: &'static str,
}

impl CanonicalPackageReconstructionQuestionError {
    const fn new(message: &'static str) -> Self {
        Self { message }
    }

    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl fmt::Display for CanonicalPackageReconstructionQuestionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for CanonicalPackageReconstructionQuestionError {}

/// Domain-separated identity of one complete reconstruction question.
///
/// This identifies the question only. It is not a discharge result, package
/// admission, accepted lock state, or proof that reconstruction occurred.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalPackageReconstructionQuestionFingerprint([u8; 32]);

impl CanonicalPackageReconstructionQuestionFingerprint {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for CanonicalPackageReconstructionQuestionFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl fmt::Display for CanonicalPackageReconstructionQuestionFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// One exact package-to-obligation association within the source closure.
///
/// Construction is private to the complete question so callers cannot splice
/// a package key and unrelated ledger into an apparently checked entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalPackageReconstructionEntry {
    package: PackageKey,
    obligation_ledger: OrdinaryPackageObligationLedger,
}

impl CanonicalPackageReconstructionEntry {
    pub const fn package(&self) -> &PackageKey {
        &self.package
    }

    pub const fn obligation_ledger(&self) -> &OrdinaryPackageObligationLedger {
        &self.obligation_ledger
    }
}

/// Canonical, non-admitting association of exact source selection with every
/// package's independently reconstructed ordinary obligation question.
///
/// The complete source-subject and ledger bytes are retained, rather than only
/// their fingerprints. Compiler executable identity, source coordinates,
/// build observations, certificates, results, open obligations, and policy
/// decisions remain separate. Recovery validates framing and association only;
/// use requires fresh source resolution and package-aware compilation followed
/// by exact reconstruction through `matches_resolved_and_reviews`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalPackageReconstructionQuestion {
    source_closure: CanonicalSourceClosureSubject,
    entries: Vec<CanonicalPackageReconstructionEntry>,
    canonical_bytes: Vec<u8>,
    fingerprint: CanonicalPackageReconstructionQuestionFingerprint,
}

impl CanonicalPackageReconstructionQuestion {
    /// Associate one freshly resolved source closure with the complete review
    /// set produced by package-aware local compilation.
    ///
    /// `CompilerIssuedPackageReviewSet` has no public constructor and each of
    /// its ledgers has already passed exact local reconstruction. This method
    /// additionally rejoins every review to resolver identity, immutable
    /// resolution, and the exact transitive source graph.
    pub fn from_resolved_and_reviews(
        closure: &ResolvedPackageSourceClosure,
        reviews: &CompilerIssuedPackageReviewSet,
        limits: CanonicalPackageReconstructionQuestionLimits,
    ) -> Result<Self, CanonicalPackageReconstructionQuestionError> {
        let limits = limits.compiler_bounded();
        let source_closure =
            CanonicalSourceClosureSubject::from_resolved(closure, limits.source_closure).map_err(
                |_| {
                    CanonicalPackageReconstructionQuestionError::new(
                        "could not project the canonical source-closure subject",
                    )
                },
            )?;

        let mut reviews_by_package = BTreeMap::new();
        for review in reviews.reviews() {
            if reviews_by_package.insert(review.key(), review).is_some() {
                return Err(CanonicalPackageReconstructionQuestionError::new(
                    "package review set contains a duplicate package",
                ));
            }
        }
        if reviews_by_package.len() != source_closure.packages().len() {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "source closure and package review set are not bijective",
            ));
        }

        let mut entries = Vec::new();
        entries
            .try_reserve_exact(source_closure.packages().len())
            .map_err(|_| {
                CanonicalPackageReconstructionQuestionError::new(
                    "package reconstruction entry allocation failed",
                )
            })?;
        for selected in source_closure.packages() {
            let review = reviews_by_package.remove(selected.key()).ok_or_else(|| {
                CanonicalPackageReconstructionQuestionError::new(
                    "source package has no matching package review",
                )
            })?;
            if review.resolution() != selected.resolution() {
                return Err(CanonicalPackageReconstructionQuestionError::new(
                    "package review immutable resolution does not match source custody",
                ));
            }
            let expected_dependency_closure =
                package_compilation_inputs_for(closure, selected.key())
                    .map_err(|_| {
                        CanonicalPackageReconstructionQuestionError::new(
                            "could not independently reconstruct the package dependency closure",
                        )
                    })?
                    .dependency_closure();
            if review.obligation_ledger().dependency_closure() != &expected_dependency_closure {
                return Err(CanonicalPackageReconstructionQuestionError::new(
                    "package review dependency closure does not match current source custody",
                ));
            }
            entries.push(CanonicalPackageReconstructionEntry {
                package: selected.key().clone(),
                obligation_ledger: review.obligation_ledger().clone(),
            });
        }
        if !reviews_by_package.is_empty() {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "package review set contains a package outside the source closure",
            ));
        }

        Self::finish(source_closure, entries, limits)
    }

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

    pub const fn source_closure(&self) -> &CanonicalSourceClosureSubject {
        &self.source_closure
    }

    pub fn entries(&self) -> &[CanonicalPackageReconstructionEntry] {
        &self.entries
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn fingerprint(&self) -> CanonicalPackageReconstructionQuestionFingerprint {
        self.fingerprint
    }

    pub fn target_name(&self) -> &'static str {
        self.entries[0].obligation_ledger.target().target_name()
    }

    /// Reproject the complete question from current resolver custody and fresh
    /// package-aware reviews, then require exact equality.
    pub fn matches_resolved_and_reviews(
        &self,
        closure: &ResolvedPackageSourceClosure,
        reviews: &CompilerIssuedPackageReviewSet,
        limits: CanonicalPackageReconstructionQuestionLimits,
    ) -> Result<bool, CanonicalPackageReconstructionQuestionError> {
        Ok(self == &Self::from_resolved_and_reviews(closure, reviews, limits)?)
    }

    fn finish(
        source_closure: CanonicalSourceClosureSubject,
        entries: Vec<CanonicalPackageReconstructionEntry>,
        limits: CanonicalPackageReconstructionQuestionLimits,
    ) -> Result<Self, CanonicalPackageReconstructionQuestionError> {
        let limits = limits.compiler_bounded();
        validate_association(&source_closure, &entries, limits)?;
        let canonical_bytes = encode_question(&source_closure, &entries, limits)?;
        let fingerprint = fingerprint(&canonical_bytes);
        Ok(Self {
            source_closure,
            entries,
            canonical_bytes,
            fingerprint,
        })
    }
}

fn validate_association(
    source_closure: &CanonicalSourceClosureSubject,
    entries: &[CanonicalPackageReconstructionEntry],
    limits: CanonicalPackageReconstructionQuestionLimits,
) -> Result<(), CanonicalPackageReconstructionQuestionError> {
    if source_closure.packages().is_empty() {
        return Err(CanonicalPackageReconstructionQuestionError::new(
            "package reconstruction source closure is empty",
        ));
    }
    if entries.len() != source_closure.packages().len() {
        return Err(CanonicalPackageReconstructionQuestionError::new(
            "source closure and obligation ledgers are not bijective",
        ));
    }
    if entries.len() > limits.maximum_packages {
        return Err(CanonicalPackageReconstructionQuestionError::new(
            "package reconstruction question exceeds its package-count ceiling",
        ));
    }

    let mut identities = BTreeMap::<PackageKeyIdentity, &PackageKey>::new();
    for source in source_closure.packages() {
        if identities
            .insert(source.key().identity(), source.key())
            .is_some()
        {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "distinct package keys collide on compiler package identity",
            ));
        }
    }

    let expected_target = entries[0].obligation_ledger.target();
    let mut total_ledger_bytes = 0usize;
    for (source, entry) in source_closure.packages().iter().zip(entries) {
        if entry.package != *source.key() {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "package reconstruction entries are not in canonical source-package order",
            ));
        }
        if entry.obligation_ledger.package() != entry.package.identity() {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "obligation ledger root identity does not match its source package",
            ));
        }
        if entry.obligation_ledger.target() != expected_target {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "package reconstruction question mixes deployment targets",
            ));
        }
        validate_ledger_source_closure(source_closure, entry)?;
        let encoded =
            encode_ordinary_package_obligation_ledger(&entry.obligation_ledger).map_err(|_| {
                CanonicalPackageReconstructionQuestionError::new(
                    "package reconstruction question contains an invalid obligation ledger",
                )
            })?;
        if encoded.len() > limits.maximum_ledger_bytes {
            return Err(CanonicalPackageReconstructionQuestionError::new(
                "package reconstruction obligation ledger exceeds its byte ceiling",
            ));
        }
        total_ledger_bytes = total_ledger_bytes
            .checked_add(encoded.len())
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
    }
    Ok(())
}

fn validate_ledger_source_closure(
    source_closure: &CanonicalSourceClosureSubject,
    entry: &CanonicalPackageReconstructionEntry,
) -> Result<(), CanonicalPackageReconstructionQuestionError> {
    let reachable = reachable_source_packages(source_closure, &entry.package);
    let mut expected_packages = reachable
        .iter()
        .map(PackageKey::identity)
        .collect::<Vec<_>>();
    expected_packages.sort_unstable();
    if entry.obligation_ledger.dependency_closure().packages() != expected_packages {
        return Err(CanonicalPackageReconstructionQuestionError::new(
            "obligation ledger package closure does not match the source subject",
        ));
    }

    let mut expected_dependencies = source_closure
        .dependency_requests()
        .iter()
        .filter(|dependency| reachable.contains(dependency.requester()))
        .map(|dependency| {
            (
                dependency.requester().identity(),
                dependency.alias().as_str(),
                dependency.selected().key().identity(),
            )
        })
        .collect::<Vec<_>>();
    expected_dependencies.sort_unstable();
    let actual_dependencies = entry
        .obligation_ledger
        .dependency_closure()
        .dependencies()
        .iter()
        .map(|dependency| {
            (
                dependency.requester(),
                dependency.alias(),
                dependency.target(),
            )
        });
    if !expected_dependencies.into_iter().eq(actual_dependencies) {
        return Err(CanonicalPackageReconstructionQuestionError::new(
            "obligation ledger dependency edges do not match the source subject",
        ));
    }
    Ok(())
}

fn reachable_source_packages(
    source_closure: &CanonicalSourceClosureSubject,
    root: &PackageKey,
) -> BTreeSet<PackageKey> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![root.clone()];
    while let Some(package) = pending.pop() {
        if !reachable.insert(package.clone()) {
            continue;
        }
        pending.extend(
            source_closure
                .dependency_requests()
                .iter()
                .filter(|dependency| dependency.requester() == &package)
                .map(|dependency| dependency.selected().key().clone()),
        );
    }
    reachable
}

fn encode_question(
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

fn fingerprint(bytes: &[u8]) -> CanonicalPackageReconstructionQuestionFingerprint {
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
