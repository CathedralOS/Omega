//! Custody-substitution machinery for the installed admission-row matrix.
//!
//! `omega.admissions` is the persisted owner-policy record: canonically
//! ordered `digest  commitment` rows installed by explicit acceptance and
//! replayed by `read_trust_admissions` + `settle_trust_admissions` at
//! settlement. The matrix declares its substitution lanes as
//! `*FieldForTest` inventories consumed by the shared
//! `run_one_field_substitution_matrix` driver:
//!
//! - `InstalledAdmissionRowFieldForTest` covers the well-formed lanes —
//!   per-field substitutions and canonical-position roster mutations that
//!   still decode, so the settlement replay must reject them with an exact
//!   unresolved/unused verdict.
//! - `AdmissionFileEncodingFieldForTest` covers the canonical-encoding lanes
//!   — mutations that cannot join the decoding grammar, so the read rejects
//!   with the named diagnostic arm and explicit acceptance must preserve the
//!   unrecognized contents byte-exact.
//!
//! `report_identity` is never persisted, so it is not a representable
//! substitution axis; the report-coordinate independence assertion stays in
//! the test itself.

use crate::admission_policy::{accept_trust_admissions, read_trust_admissions};
use effects::provider_plan::ProviderPlanDigest;
use optimization_core::MutationOutcome;
use std::path::PathBuf;
use trust_model::{TrustAdmission, TrustAdmissionDigest, settle_trust_admissions};

/// Persisted admission at a fixed digest byte — the file's foreign side of
/// any settlement disagreement.
pub fn persisted(commitment: &str, byte: u8) -> TrustAdmission {
    TrustAdmission::from_persisted(
        commitment.to_owned(),
        TrustAdmissionDigest::from_digest([byte; 32]).unwrap(),
    )
    .unwrap()
}

/// Persisted admission carrying an exact recomputed digest.
pub fn persisted_with_digest(commitment: &str, digest: TrustAdmissionDigest) -> TrustAdmission {
    TrustAdmission::from_persisted(commitment.to_owned(), digest).unwrap()
}

/// Honestly reconstructed obligation: the digest is derived from the
/// admitted subject's own identity, never read back out of the file.
pub fn derived(commitment: &str, report_identity: u64, plan_byte: u8) -> TrustAdmission {
    TrustAdmission::for_provider_plan(
        commitment.to_owned(),
        report_identity,
        ProviderPlanDigest::from_digest([plan_byte; 32]),
    )
    .unwrap()
}

optimization_core::custody_field_inventory! {
    /// One well-formed substitution lane of the persisted `omega.admissions`
    /// roster: the digest field, the commitment field, the swapped-digest
    /// pairing, and the canonical-position forged-insert and drop axes. Each
    /// substitution decodes cleanly and must be rejected by the settlement
    /// replay in both directions — unused foreign row, unresolved obligation.
    pub enum InstalledAdmissionRowFieldForTest {
        RowZeroDigest,
        RowZeroCommitment,
        DigestSwapRowsZeroOne,
        ForgedRowInserted,
        RowDropped,
    }
}

optimization_core::custody_field_inventory! {
    /// One canonical-encoding lane of the `omega.admissions` record: content
    /// mutations that cannot join the decode grammar — ordering violations,
    /// duplicated or malformed rows, legacy compact digests, and framing
    /// breaks. Every lane rejects at `read_trust_admissions` decoding, and
    /// `accept_trust_admissions` must refuse to repair or replace the
    /// unrecognized custody byte-exact.
    pub enum AdmissionFileEncodingFieldForTest {
        ReorderedRows,
        DuplicatedRow,
        CommitmentBreaksOrder,
        LegacyCompactDigest,
        ZeroDigest,
        NonHexDigest,
        EmptyCommitment,
        MissingFieldSeparator,
        CarriageReturnInCommitment,
        MissingTrailingNewline,
        TrailingJunkRow,
    }
}

/// The settlement verdict a substitution must produce: the exact required
/// obligations left unresolved and the exact persisted rows left unused.
#[derive(Debug, Clone, PartialEq)]
pub struct AdmissionSettlementVerdict {
    pub unresolved: Vec<TrustAdmission>,
    pub unused: Vec<TrustAdmission>,
}

/// How a substituted installed policy rejects.
#[derive(Debug, Clone, PartialEq)]
pub enum InstalledAdmissionRejection {
    /// The policy decoded but the settlement verdict mismatches.
    Unsettled(AdmissionSettlementVerdict),
    /// The substituted contents could not decode — never a legitimate
    /// outcome for a well-formed row substitution.
    Undecodable,
}

/// Which named decode arm a non-canonical file must reject with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionEncodingRejection {
    CanonicalOrderViolation,
    DuplicateCommitment,
    LegacyCompactRow,
    MalformedRow,
    Unclassified,
}

/// The canonical three-row required set: the admitted obligations the honest
/// policy settles exactly.
pub fn canonical_required_admissions() -> Vec<TrustAdmission> {
    vec![
        derived("accepted fact: Alpha", 1, 0x11),
        derived("provider slot: Beta", 2, 0x22),
        derived("provider slot: Gamma", 3, 0x33),
    ]
}

/// `(digest, commitment)` rows in canonical file order.
pub fn canonical_admission_rows(required: &[TrustAdmission]) -> Vec<(String, String)> {
    required
        .iter()
        .map(|admission| {
            (
                admission.digest().to_string(),
                admission.commitment().to_owned(),
            )
        })
        .collect()
}

/// Render persisted rows in their given order — the file's canonical
/// serialization when the roster is itself canonical.
fn render_rows(rows: &[(String, String)]) -> String {
    rows.iter()
        .map(|(digest, commitment)| format!("{digest}  {commitment}\n"))
        .collect()
}

/// The substitution project directory (per-process, stable across cases —
/// each case writes its own contents before checking, so sharing one root
/// keeps the matrix legs sequential and deterministic).
pub fn substitution_root(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("create substitution project");
    root
}

/// The record a well-formed row substitution mutates: the substitution
/// project root, the required obligation set, and the persisted roster rows.
pub struct InstalledAdmissionCase {
    pub root: PathBuf,
    pub required: Vec<TrustAdmission>,
    pub rows: Vec<(String, String)>,
}

/// The honestly produced installed policy: the canonical required set
/// accepted explicitly, with the persisted rows read back verbatim.
pub fn honest_installed_admission_case() -> InstalledAdmissionCase {
    let root = substitution_root("omega-admissions-one-field");
    let root_path = root.join("main.omg");
    let required = canonical_required_admissions();
    accept_trust_admissions(&root_path, &required).expect("install canonical policy");
    let canonical =
        std::fs::read_to_string(root.join("omega.admissions")).expect("read installed policy");
    let rows = canonical_admission_rows(&required);
    assert_eq!(render_rows(&rows), canonical);
    InstalledAdmissionCase {
        root,
        required,
        rows,
    }
}

/// An authentic foreign roster of the same family: the canonical rows with a
/// forged row inserted at canonical position — decodable, but custody that
/// never settled this required set.
pub fn foreign_installed_admission_donor() -> InstalledAdmissionCase {
    let mut case = honest_installed_admission_case();
    case.rows
        .insert(1, ("5".repeat(64), "accepted fact: Foreign".to_owned()));
    case
}

/// Mutate exactly one declared lane of the persisted roster. Every alternate
/// is fixed and well-formed: the field's foreign value or the canonical
/// roster-shape change itself is the substitution.
pub fn corrupt_installed_admission_for_test(
    case: &mut InstalledAdmissionCase,
    field: InstalledAdmissionRowFieldForTest,
    _donor: &InstalledAdmissionCase,
) {
    use InstalledAdmissionRowFieldForTest as Field;
    match field {
        Field::RowZeroDigest => case.rows[0].0 = "9".repeat(64),
        Field::RowZeroCommitment => case.rows[0].1 = "accepted fact: Alps".to_owned(),
        Field::DigestSwapRowsZeroOne => {
            case.rows.swap(0, 1);
            // Keep the commitments in canonical file order; only the digest
            // lane crossed rows.
            let commitment_zero = case.rows[0].1.clone();
            case.rows[0].1 = case.rows[1].1.clone();
            case.rows[1].1 = commitment_zero;
        }
        Field::ForgedRowInserted => {
            case.rows
                .insert(1, ("7".repeat(64), "accepted fact: Mid".to_owned()));
        }
        Field::RowDropped => {
            case.rows.remove(1);
        }
    }
}

/// The family's independent checker: write the mutated roster, decode it,
/// and settle the required obligations against it.
pub fn check_installed_admission_custody(
    case: &InstalledAdmissionCase,
) -> Result<Vec<(String, String)>, InstalledAdmissionRejection> {
    let root_path = case.root.join("main.omg");
    std::fs::write(case.root.join("omega.admissions"), render_rows(&case.rows))
        .expect("write mutated policy");
    let accepted = match read_trust_admissions(&root_path) {
        Ok(accepted) => accepted,
        Err(_) => return Err(InstalledAdmissionRejection::Undecodable),
    };
    match settle_trust_admissions(case.required.clone(), &accepted) {
        Ok(settlement) if settlement.is_exactly_admitted() => Ok(case.rows.clone()),
        Ok(settlement) => Err(InstalledAdmissionRejection::Unsettled(
            AdmissionSettlementVerdict {
                unresolved: settlement.unresolved().to_vec(),
                unused: settlement.unused().to_vec(),
            },
        )),
        Err(_) => Err(InstalledAdmissionRejection::Undecodable),
    }
}

/// The exact settlement verdict each well-formed substitution must produce.
pub fn installed_admission_row_outcome(
    field: InstalledAdmissionRowFieldForTest,
) -> MutationOutcome<InstalledAdmissionRejection> {
    use InstalledAdmissionRowFieldForTest as Field;
    let required = canonical_required_admissions();
    let verdict = |unresolved: Vec<TrustAdmission>, unused: Vec<TrustAdmission>| {
        MutationOutcome::ExactError(InstalledAdmissionRejection::Unsettled(
            AdmissionSettlementVerdict { unresolved, unused },
        ))
    };
    match field {
        // A foreign well-formed digest on the same commitment is unadmitted
        // and leaves the real row unsettled.
        Field::RowZeroDigest => verdict(
            vec![required[0].clone()],
            vec![persisted("accepted fact: Alpha", 0x99)],
        ),
        // The real digest under a different commitment text.
        Field::RowZeroCommitment => verdict(
            vec![required[0].clone()],
            vec![persisted_with_digest(
                "accepted fact: Alps",
                required[0].digest(),
            )],
        ),
        // Swapped digests keep commitment order canonical yet mismatch every
        // obligation they touch.
        Field::DigestSwapRowsZeroOne => verdict(
            vec![required[0].clone(), required[1].clone()],
            vec![
                persisted_with_digest("accepted fact: Alpha", required[1].digest()),
                persisted_with_digest("provider slot: Beta", required[0].digest()),
            ],
        ),
        // A forged extra row in canonical position is unadmitted.
        Field::ForgedRowInserted => verdict(vec![], vec![persisted("accepted fact: Mid", 0x77)]),
        // A dropped row leaves its obligation unresolved.
        Field::RowDropped => verdict(vec![required[1].clone()], vec![]),
    }
}

/// The record a canonical-encoding substitution mutates: the substitution
/// project root, the canonical row roster the contents derive from, and the
/// exact persisted bytes.
pub struct AdmissionEncodingCase {
    pub root: PathBuf,
    pub required: Vec<TrustAdmission>,
    pub base_rows: Vec<(String, String)>,
    pub contents: String,
}

/// The honestly produced encoding case: canonical rows rendered canonically.
pub fn honest_admission_encoding_case() -> AdmissionEncodingCase {
    let root = substitution_root("omega-admissions-encoding");
    let required = canonical_required_admissions();
    let base_rows = canonical_admission_rows(&required);
    let contents = render_rows(&base_rows);
    std::fs::write(root.join("omega.admissions"), &contents).expect("write canonical policy");
    AdmissionEncodingCase {
        root,
        required,
        base_rows,
        contents,
    }
}

/// An authentic foreign encoding record: the canonical rows in a
/// non-canonical order — a real `omega.admissions` payload whose retained
/// custody differs from the honest encoding.
pub fn foreign_admission_encoding_donor() -> AdmissionEncodingCase {
    let mut case = honest_admission_encoding_case();
    case.contents = render_rows(&[
        case.base_rows[1].clone(),
        case.base_rows[0].clone(),
        case.base_rows[2].clone(),
    ]);
    case
}

/// Mutate exactly one canonical-encoding lane of the persisted bytes. Lanes
/// reachable through the row grammar mutate rows and re-render; framing and
/// separator lanes edit the serialized bytes directly.
pub fn corrupt_admission_encoding_for_test(
    case: &mut AdmissionEncodingCase,
    field: AdmissionFileEncodingFieldForTest,
    _donor: &AdmissionEncodingCase,
) {
    use AdmissionFileEncodingFieldForTest as Field;
    let canonical = render_rows(&case.base_rows);
    let mut rows = case.base_rows.clone();
    case.contents = match field {
        Field::ReorderedRows => {
            rows.swap(0, 1);
            render_rows(&rows)
        }
        Field::DuplicatedRow => {
            rows.insert(1, rows[0].clone());
            render_rows(&rows)
        }
        Field::CommitmentBreaksOrder => {
            rows[0].1 = "zz topmost".to_owned();
            render_rows(&rows)
        }
        Field::LegacyCompactDigest => {
            rows[0].0 = rows[0].0[..16].to_owned();
            render_rows(&rows)
        }
        Field::ZeroDigest => {
            rows[0].0 = "0".repeat(64);
            render_rows(&rows)
        }
        Field::NonHexDigest => {
            rows[0].0.replace_range(63.., "g");
            render_rows(&rows)
        }
        Field::EmptyCommitment => {
            rows[0].1 = String::new();
            render_rows(&rows)
        }
        Field::MissingFieldSeparator => {
            format!("{} accepted fact: Alpha\n", case.base_rows[0].0)
        }
        Field::CarriageReturnInCommitment => {
            rows[0].1 = "accepted fact: Al\rpha".to_owned();
            render_rows(&rows)
        }
        Field::MissingTrailingNewline => canonical.trim_end_matches('\n').to_owned(),
        Field::TrailingJunkRow => format!("{canonical}junk\n"),
    };
}

/// Classify the decode rejection by the diagnostic arm that fired, exactly
/// as the legacy loop asserted the fragment.
fn classify_encoding_rejection(
    diagnostics: &[diagnostics::Diagnostic],
) -> AdmissionEncodingRejection {
    let rendered = format!("{diagnostics:?}");
    if rendered.contains("not in canonical commitment order") {
        AdmissionEncodingRejection::CanonicalOrderViolation
    } else if rendered.contains("duplicate commitment") {
        AdmissionEncodingRejection::DuplicateCommitment
    } else if rendered.contains("legacy 16-hex compact admission row") {
        AdmissionEncodingRejection::LegacyCompactRow
    } else if rendered.contains("malformed strong admission row") {
        AdmissionEncodingRejection::MalformedRow
    } else {
        AdmissionEncodingRejection::Unclassified
    }
}

/// The family's independent checker for encoding lanes: write the mutated
/// bytes and decode them.
pub fn check_admission_encoding(
    case: &AdmissionEncodingCase,
) -> Result<String, AdmissionEncodingRejection> {
    std::fs::write(case.root.join("omega.admissions"), &case.contents)
        .expect("write noncanonical policy");
    match read_trust_admissions(&case.root.join("main.omg")) {
        Ok(_) => Ok(case.contents.clone()),
        Err(diagnostics) => Err(classify_encoding_rejection(&diagnostics)),
    }
}

/// Every encoding lane rejects at decoding with its named arm.
pub fn admission_encoding_outcome(
    field: AdmissionFileEncodingFieldForTest,
) -> MutationOutcome<AdmissionEncodingRejection> {
    use AdmissionFileEncodingFieldForTest as Field;
    let rejection = match field {
        Field::ReorderedRows | Field::CommitmentBreaksOrder => {
            AdmissionEncodingRejection::CanonicalOrderViolation
        }
        Field::DuplicatedRow => AdmissionEncodingRejection::DuplicateCommitment,
        Field::LegacyCompactDigest => AdmissionEncodingRejection::LegacyCompactRow,
        _ => AdmissionEncodingRejection::MalformedRow,
    };
    MutationOutcome::ExactError(rejection)
}

/// Per-leg joined assertion for encoding lanes: explicit acceptance refuses
/// to repair or replace unrecognized custody, byte-exact.
pub fn admission_encoding_joined_replay(
    case: &AdmissionEncodingCase,
    field: AdmissionFileEncodingFieldForTest,
) {
    assert!(
        accept_trust_admissions(&case.root.join("main.omg"), &case.required).is_err(),
        "{field:?}: acceptance must preserve an unrecognized policy"
    );
    assert_eq!(
        std::fs::read_to_string(case.root.join("omega.admissions")).expect("policy preserved"),
        case.contents,
        "{field:?}: rejected contents stay byte-exact"
    );
}
