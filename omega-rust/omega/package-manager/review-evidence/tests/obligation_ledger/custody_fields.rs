//! Substitutable-field inventory for the ordinary package obligation ledger.
//!
//! The family's declared legs are driven through the shared substitution
//! matrix by `obligation_ledger.rs`; keeping the inventory in its own file
//! names the boundary between what is declared substitutable and the driver
//! that exercises every leg. The record is the ledger's canonical encoding:
//! `OrdinaryPackageObligationLedger` exposes no field mutation surface, so one
//! authored leg splices one field of the retained bytes through the exact
//! `LedgerFieldSpans` coordinates and must still decode to a canonical,
//! differently-fingerprinted ledger. Framing magic, encoding and vocabulary
//! versions, subject coordinates, roster-order and ceiling axes, inner-row
//! framing, truncations, and trailing bytes cannot form a record and stay
//! authored in `obligation_ledger.rs` as decode- and recovery-level
//! rejections; the same file keeps the roster-level recovery axes that are
//! substitutions of the decoded row list rather than of the record itself.

use diagnostics::Diagnostic;
use optimization_core::{MutationOutcome, custody_field_inventory};
use package_compilation::{
    BuildDeclarationKind, PackageDependencyBinding, PackageDependencyClosure,
};
use package_evidence::encoding::{
    DecodedPackageReviewCanonicalRow, decode_package_review_canonical_row,
};
use package_evidence::ledger::{
    OrdinaryPackageObligationLedger, OrdinaryPackageObligationLedgerFingerprint,
    decode_ordinary_package_obligation_ledger, encode_ordinary_package_obligation_ledger,
    ordinary_package_obligation_ledger_fingerprint, recover_ordinary_package_obligation_ledger,
};
use semantic_vocabulary::PackageKeyIdentity;

use crate::support::{
    canonical_row_spans, crafted_authored_row_envelope, crafted_canonical_row,
    crafted_row_envelope, ledger_field_spans, package_identity, push_length_prefixed,
};

// Raw row wire tags; the canonical kind/risk join requires these exact pairs.
pub(crate) const KIND_PROJECTION_HEADER: u8 = 0;
pub(crate) const KIND_PUBLIC_TRAIT: u8 = 1;
pub(crate) const KIND_PUBLIC_DATA: u8 = 3;
pub(crate) const KIND_SELECTED_PROVIDER_SET: u8 = 7;
pub(crate) const KIND_PUBLIC_CONST: u8 = 12;
pub(crate) const RISK_BLOCKING: u8 = 0;
pub(crate) const RISK_OPAQUE_BLOCKING: u8 = 2;
pub(crate) const DERIVATION_PROJECTION_HEADER: u8 = 0;
const DERIVATION_EMPTY_PROVIDER_SET: u8 = 1;
const ROLE_DECLARATION: u8 = 0;

/// The crafted ledger's canonical target; the replay fixture compiles for the
/// same target so validation reaches the closure and row arms.
pub(crate) const LEDGER_TARGET: &str = "linux_x86_64";

custody_field_inventory! {
    /// One representable substitution of the encoded ordinary package
    /// obligation ledger. Each leg decodes to a canonical ledger whose
    /// honestly recomputed fingerprint diverges and whose independent
    /// reconstruction replay against a real checked compilation rejects it;
    /// the leg's outcome names which `validate_ordinary_package_obligation_ledger`
    /// custody arm reports the rejection.
    pub enum ObligationLedgerCustodyFieldForTest {
        RootRoleApplication,
        DependencyAliasRenamed,
        DependencyTargetsSwapped,
        OrdinaryRowDropped,
        RowKindSubstituted,
        RowKeySubstituted,
        RowValueSubstituted,
        ForeignRowAppended,
    }
}

/// The crafted ledger fixture: a canonical two-edge dependency closure, three
/// canonical rows issued through the public row-recovery envelope, and the
/// ledger's encoded form. Hand-encoding keeps the codec legs runnable on
/// hosts that cannot compile a package-aware fixture.
pub(crate) struct CraftedLedgerFixture {
    pub(crate) closure: PackageDependencyClosure,
    pub(crate) decoded_rows: Vec<DecodedPackageReviewCanonicalRow>,
    pub(crate) ledger: OrdinaryPackageObligationLedger,
    pub(crate) encoded: Vec<u8>,
}

pub(crate) fn crafted_obligation_ledger_fixture() -> CraftedLedgerFixture {
    let package = package_identity();
    let dependency = PackageKeyIdentity::from_digest([42; 32]).expect("dependency identity");
    let secondary = PackageKeyIdentity::from_digest([43; 32]).expect("secondary identity");

    let canonical_rows = [
        crafted_canonical_row(
            package,
            LEDGER_TARGET,
            KIND_PROJECTION_HEADER,
            RISK_BLOCKING,
            b"header",
            b"header-value",
        ),
        crafted_canonical_row(
            package,
            LEDGER_TARGET,
            KIND_PUBLIC_DATA,
            RISK_BLOCKING,
            b"data/token",
            b"token-value",
        ),
        crafted_canonical_row(
            package,
            LEDGER_TARGET,
            KIND_SELECTED_PROVIDER_SET,
            RISK_OPAQUE_BLOCKING,
            b"providers",
            b"provider-set",
        ),
    ];
    let envelopes = [
        crafted_row_envelope(&canonical_rows[0], &[DERIVATION_PROJECTION_HEADER]),
        crafted_authored_row_envelope(
            &canonical_rows[1],
            package,
            "main.omg",
            0,
            12,
            ROLE_DECLARATION,
        ),
        crafted_row_envelope(&canonical_rows[2], &[DERIVATION_EMPTY_PROVIDER_SET]),
    ];
    let decoded_rows = envelopes
        .iter()
        .map(|bytes| {
            decode_package_review_canonical_row(bytes)
                .expect("crafted row recovery envelope should decode")
        })
        .collect::<Vec<_>>();

    let closure = PackageDependencyClosure::from_canonical_parts(
        package,
        BuildDeclarationKind::Package,
        vec![package, dependency, secondary],
        vec![
            PackageDependencyBinding::new(package, "alpha_dep", dependency),
            PackageDependencyBinding::new(package, "beta_dep", secondary),
        ],
    )
    .expect("crafted dependency closure should validate");
    let ledger = recover_ordinary_package_obligation_ledger(closure.clone(), &decoded_rows)
        .expect("crafted rows should form a canonical ledger");
    let encoded =
        encode_ordinary_package_obligation_ledger(&ledger).expect("crafted ledger should encode");
    assert_eq!(
        decode_ordinary_package_obligation_ledger(&encoded).expect("crafted ledger should decode"),
        ledger
    );
    CraftedLedgerFixture {
        closure,
        decoded_rows,
        ledger,
        encoded,
    }
}

/// A substitution's verdict, named by the custody arm that reports it.
///
/// The matrix's checker decodes the substituted bytes and replays the decoded
/// ledger through `validate_ordinary_package_obligation_ledger` against a real
/// checked compilation — the fixture record was never issued by that
/// compilation, so every leg must reach a "does not match local
/// reconstruction" diagnostic. The arm names the first point of divergence in
/// the validator's schema/package/target/closure/row comparison order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ObligationLedgerCheck {
    /// The substituted bytes failed canonical decoding. No declared leg
    /// expects this: it exists so a leg that accidentally produces an
    /// unrepresentable record fails loudly with the decode message.
    DecodeRejection(String),
    /// The reconstructed schema identity diverged.
    SchemaMismatch,
    /// The reconstructed package identity diverged.
    PackageIdentityMismatch,
    /// The reconstructed target diverged.
    TargetMismatch,
    /// The reconstructed dependency closure diverged.
    DependencyClosureMismatch,
    /// Row `index` diverged from local reconstruction.
    RowMismatch(usize),
    /// The candidate's row count diverged from the reconstructed count.
    RowCountMismatch { candidate: usize, expected: usize },
    /// Replay rejected through an unclassified diagnostic; the verbatim
    /// message travels with the verdict so the failure names itself.
    OtherRejection(String),
}

/// Classify the first reconstruction diagnostic into the custody arm that
/// reported it.
pub(crate) fn classify_obligation_ledger_rejection(
    diagnostics: &[Diagnostic],
) -> ObligationLedgerCheck {
    let message = diagnostics
        .first()
        .expect("a rejected replay carries at least one diagnostic")
        .message
        .clone();
    if message.contains("schema does not match local reconstruction") {
        return ObligationLedgerCheck::SchemaMismatch;
    }
    if message.contains("package identity does not match local reconstruction") {
        return ObligationLedgerCheck::PackageIdentityMismatch;
    }
    if message.contains("target does not match local reconstruction") {
        return ObligationLedgerCheck::TargetMismatch;
    }
    if message.contains("dependency closure does not match local reconstruction") {
        return ObligationLedgerCheck::DependencyClosureMismatch;
    }
    if let Some(index) = message
        .strip_prefix("ordinary package obligation ledger row ")
        .and_then(|rest| rest.split(' ').next())
        .and_then(|index| index.parse::<usize>().ok())
    {
        return ObligationLedgerCheck::RowMismatch(index);
    }
    if let Some(counts) = message.strip_prefix("ordinary package obligation ledger row count ") {
        let mut numbers = counts.split(" does not match locally reconstructed count ");
        if let (Some(candidate), Some(expected)) = (numbers.next(), numbers.next())
            && let (Ok(candidate), Ok(expected)) =
                (candidate.parse::<usize>(), expected.parse::<usize>())
        {
            return ObligationLedgerCheck::RowCountMismatch {
                candidate,
                expected,
            };
        }
    }
    ObligationLedgerCheck::OtherRejection(message)
}

/// Apply one declared substitution to the ledger's canonical bytes.
///
/// The donor record authenticates a different issuance of the same family;
/// these legs mutate coordinates inside the honest record rather than borrow
/// donor values.
pub(crate) fn substitute_obligation_ledger_for_test(
    record: &mut Vec<u8>,
    field: ObligationLedgerCustodyFieldForTest,
    donor: &Vec<u8>,
) {
    let _ = donor;
    let spans = ledger_field_spans(record);
    match field {
        ObligationLedgerCustodyFieldForTest::RootRoleApplication => {
            // Application carries wire tag 1; Package carries 0.
            record[spans.root_role] = 1;
        }
        ObligationLedgerCustodyFieldForTest::DependencyAliasRenamed => {
            record[spans.dependencies[1].alias.clone()].copy_from_slice(b"zeta_dep");
        }
        ObligationLedgerCustodyFieldForTest::DependencyTargetsSwapped => {
            let alpha = record[spans.dependencies[0].target.clone()].to_vec();
            let beta = record[spans.dependencies[1].target.clone()].to_vec();
            record[spans.dependencies[0].target.clone()].copy_from_slice(&beta);
            record[spans.dependencies[1].target.clone()].copy_from_slice(&alpha);
        }
        ObligationLedgerCustodyFieldForTest::OrdinaryRowDropped => {
            let dropped = spans.rows[1].clone();
            let count = u64::try_from(spans.rows.len() - 1).expect("row count fits u64");
            record[spans.row_count.clone()].copy_from_slice(&count.to_le_bytes());
            record.splice(dropped, std::iter::empty());
        }
        ObligationLedgerCustodyFieldForTest::RowKindSubstituted
        | ObligationLedgerCustodyFieldForTest::RowKeySubstituted
        | ObligationLedgerCustodyFieldForTest::RowValueSubstituted => {
            let content_start = spans.rows[1].start + 8;
            let inner = canonical_row_spans(&record[content_start..spans.rows[1].end]);
            match field {
                ObligationLedgerCustodyFieldForTest::RowKindSubstituted => {
                    record[content_start + inner.kind] = KIND_PUBLIC_TRAIT;
                }
                ObligationLedgerCustodyFieldForTest::RowKeySubstituted => {
                    record[content_start + inner.key.start] = b'X';
                }
                ObligationLedgerCustodyFieldForTest::RowValueSubstituted => {
                    record[content_start + inner.value.start] ^= 0x01;
                }
                _ => unreachable!("row-content arm only"),
            }
        }
        ObligationLedgerCustodyFieldForTest::ForeignRowAppended => {
            let package = PackageKeyIdentity::from_digest(
                <[u8; 32]>::try_from(&record[spans.package.clone()]).expect("package digest"),
            )
            .expect("ledger package identity");
            let target =
                std::str::from_utf8(&record[spans.target.clone()]).expect("ledger target is UTF-8");
            let forged = crafted_canonical_row(
                package,
                target,
                KIND_PUBLIC_CONST,
                RISK_BLOCKING,
                b"extra",
                b"v",
            );
            let count = u64::try_from(spans.rows.len() + 1).expect("row count fits u64");
            record[spans.row_count.clone()].copy_from_slice(&count.to_le_bytes());
            push_length_prefixed(record, &forged);
        }
    }
}

/// The containing identity: the fingerprint honestly recomputed over the
/// decoded ledger.
pub(crate) fn obligation_ledger_custody(
    record: &[u8],
) -> OrdinaryPackageObligationLedgerFingerprint {
    let ledger =
        decode_ordinary_package_obligation_ledger(record).expect("a matrix leg must decode");
    ordinary_package_obligation_ledger_fingerprint(&ledger).expect("ledger fingerprint")
}

/// Name the verdict a declared leg must produce at reconstruction replay.
pub(crate) fn obligation_ledger_custody_outcome(
    field: ObligationLedgerCustodyFieldForTest,
) -> MutationOutcome<ObligationLedgerCheck> {
    MutationOutcome::ExactError(match field {
        ObligationLedgerCustodyFieldForTest::RootRoleApplication
        | ObligationLedgerCustodyFieldForTest::DependencyAliasRenamed
        | ObligationLedgerCustodyFieldForTest::DependencyTargetsSwapped => {
            ObligationLedgerCheck::DependencyClosureMismatch
        }
        ObligationLedgerCustodyFieldForTest::OrdinaryRowDropped
        | ObligationLedgerCustodyFieldForTest::ForeignRowAppended => {
            ObligationLedgerCheck::RowMismatch(0)
        }
        ObligationLedgerCustodyFieldForTest::RowKindSubstituted
        | ObligationLedgerCustodyFieldForTest::RowKeySubstituted
        | ObligationLedgerCustodyFieldForTest::RowValueSubstituted => {
            ObligationLedgerCheck::RowMismatch(0)
        }
    })
}
