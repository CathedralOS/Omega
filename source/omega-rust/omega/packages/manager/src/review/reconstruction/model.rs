use crate::resolution::graph::{CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits};
use crate::manifest::PackageKey;
use omega_package_evidence::obligations::OrdinaryPackageObligationLedger;
use std::fmt;

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
    pub(super) fn compiler_bounded(self) -> Self {
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
    pub(super) const fn new(message: &'static str) -> Self {
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
pub struct CanonicalPackageReconstructionQuestionFingerprint(pub(super) [u8; 32]);

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
    pub(super) package: PackageKey,
    pub(super) obligations: OrdinaryPackageObligationLedger,
}

impl CanonicalPackageReconstructionEntry {
    pub const fn package(&self) -> &PackageKey {
        &self.package
    }

    pub const fn obligations(&self) -> &OrdinaryPackageObligationLedger {
        &self.obligations
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
    pub(super) source_closure: CanonicalSourceClosureSubject,
    pub(super) entries: Vec<CanonicalPackageReconstructionEntry>,
    pub(super) canonical_bytes: Vec<u8>,
    pub(super) fingerprint: CanonicalPackageReconstructionQuestionFingerprint,
}

impl CanonicalPackageReconstructionQuestion {
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
        self.entries[0].obligations.target().target_name()
    }
}
