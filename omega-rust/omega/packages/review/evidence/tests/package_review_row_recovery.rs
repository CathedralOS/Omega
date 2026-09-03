use omega_compiler::compile_to_checked_with_packages;
use omega_package_compilation::{PackageCompilationInputs, PackageSourceBinding};
use omega_package_evidence::encoding::{
    PACKAGE_REVIEW_CANONICAL_ROW_RECOVERY_VERSION, PackageReviewCanonicalRowRecoveryLimits,
    decode_package_review_canonical_row, decode_package_review_canonical_row_with_limits,
    encode_package_review_canonical_row,
};
use omega_package_evidence::project_checked_package_review;
use omega_package_evidence::record::{
    PackageReviewCanonicalRowKind, PackageReviewSourceLocationRole,
};
use psi_core::PackageKeyIdentity;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const RECOVERY_MAGIC: &[u8] = b"OMEGA-PACKAGE-REVIEW-ROW-RECOVERY\0";
const ROW_MAGIC: &[u8] = b"OMEGA-PACKAGE-REVIEW-ROW\0";

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempPackage(PathBuf);

impl TempPackage {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-package-evidence-row-recovery-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create package review row recovery fixture");
        Self(path)
    }

    fn write(&self, path: impl AsRef<Path>, source: &str) {
        fs::write(self.0.join(path), source).expect("write package review row recovery fixture");
    }
}

impl Drop for TempPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn package_identity() -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([0x6d; 32]).expect("nonzero package identity")
}

fn fixture_rows() -> Option<(
    omega_target::TargetProfile,
    Vec<omega_package_evidence::record::PackageReviewCanonicalRow>,
)> {
    let target_name = host_target_name()?;
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Token { value: i64; }
pub trait Marker { machine Self::touch(&self, value: i64); }
pub Primary: Token satisfies Marker { machine touch(&self, value: i64) { } }
pub proposition ready() = true;
pub const LIMIT: u64 = 4;
pub domain Token::Nonnegative requires self.value >= 0;
pub boundary trait ForeignSurface {
    machine invoke() reaches ForeignSurface;
}
pub machine invoke_leaf()
    satisfies ForeignSurface::invoke
    via Binding::DllImport("omega-host", "invoke");
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let inputs = PackageCompilationInputs::new_package(
        package_identity(),
        vec![PackageSourceBinding::new(
            package_identity(),
            "review-fixture",
            package.0.clone(),
        )],
        Vec::new(),
    )
    .expect("single-package graph");
    let checked =
        compile_to_checked_with_packages(&package.0.join("main.omg"), Some(target_name), inputs)
            .expect("row recovery fixture should check");
    let review = project_checked_package_review(&checked).expect("fixture review should close");
    Some((
        review.target(),
        review.canonical_rows().expect("fixture canonical rows"),
    ))
}

#[test]
fn canonical_rows_round_trip_with_validated_package_target_and_exact_source() {
    let Some((target, rows)) = fixture_rows() else {
        return;
    };
    assert!(
        rows.iter()
            .any(|row| { row.source().authored_locations().is_some() })
    );
    assert!(
        rows.iter()
            .any(|row| { !row.source().compiler_derivations().is_empty() })
    );
    assert!(
        rows.iter()
            .any(|row| row.kind() == PackageReviewCanonicalRowKind::PublicProposition),
        "the new public proposition row kind must survive canonical recovery"
    );
    assert!(rows.iter().any(|row| {
        row.kind() == PackageReviewCanonicalRowKind::PublicProposition
            && row.source().authored_locations().is_some_and(|locations| {
                locations.iter().any(|location| {
                    location.role() == PackageReviewSourceLocationRole::PropositionFormula
                })
            })
    }));
    assert!(rows.iter().any(|row| {
        row.kind() == PackageReviewCanonicalRowKind::PublicDomain
            && row.source().authored_locations().is_some_and(|locations| {
                locations
                    .iter()
                    .any(|location| location.role() == PackageReviewSourceLocationRole::ProofFact)
            })
    }));
    assert!(rows.iter().any(|row| {
        row.kind() == PackageReviewCanonicalRowKind::PublicTrait
            && row.source().authored_locations().is_some_and(|locations| {
                locations.iter().any(|location| {
                    location.role() == PackageReviewSourceLocationRole::TraitRequirement
                })
            })
    }));
    assert!(rows.iter().any(|row| {
        row.kind() == PackageReviewCanonicalRowKind::PublicTrait
            && row.source().authored_locations().is_some_and(|locations| {
                locations.iter().any(|location| {
                    location.role() == PackageReviewSourceLocationRole::CallableParameter
                })
            })
    }));
    assert!(rows.iter().any(|row| {
        row.kind() == PackageReviewCanonicalRowKind::PublicData
            && row.source().authored_locations().is_some_and(|locations| {
                locations
                    .iter()
                    .any(|location| location.role() == PackageReviewSourceLocationRole::DataMember)
            })
    }));
    assert!(
        rows.iter()
            .any(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConst),
        "the public const row kind must survive canonical recovery"
    );
    assert!(rows.iter().any(|row| {
        row.kind() == PackageReviewCanonicalRowKind::PublicConst
            && row.source().authored_locations().is_some_and(|locations| {
                locations.iter().any(|location| {
                    location.role() == PackageReviewSourceLocationRole::ConstInitializer
                })
            })
    }));
    assert!(
        rows.iter()
            .any(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConformance),
        "the public conformance row kind must survive canonical recovery"
    );
    assert!(
        rows.iter()
            .any(|row| row.kind() == PackageReviewCanonicalRowKind::ExternalExecutableSupply),
        "the external executable-supply row kind must survive canonical recovery"
    );
    assert!(rows.iter().any(|row| {
        row.kind() == PackageReviewCanonicalRowKind::ExternalExecutableSupply
            && row.source().authored_locations().is_some_and(|locations| {
                locations.iter().any(|location| {
                    location.role() == PackageReviewSourceLocationRole::ExternalBinding
                })
            })
    }));
    assert_eq!(PACKAGE_REVIEW_CANONICAL_ROW_RECOVERY_VERSION, 23);

    for row in rows {
        let envelope = encode_package_review_canonical_row(&row).expect("encode recovery row");
        let decoded = decode_package_review_canonical_row(&envelope).expect("decode recovery row");
        assert_eq!(decoded.package(), package_identity());
        assert_eq!(decoded.target(), target);
        assert_eq!(decoded.kind(), row.kind());
        assert_eq!(decoded.risk(), row.risk());
        assert_eq!(decoded.key_bytes(), row.key_bytes());
        assert_eq!(decoded.canonical_bytes(), row.canonical_bytes());
        assert_eq!(decoded.source(), row.source());
    }
}

#[test]
fn decoder_rejects_malformed_noncanonical_and_over_limit_recovery_rows() {
    let Some((_target, rows)) = fixture_rows() else {
        return;
    };
    let authored = rows
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConformance)
        .expect("authored public-conformance row");
    let envelope = encode_package_review_canonical_row(authored).expect("encode authored row");

    let mut malformed = envelope.clone();
    malformed.pop();
    assert!(decode_package_review_canonical_row(&malformed).is_err());

    let mut malformed = envelope.clone();
    malformed.push(0);
    assert!(decode_package_review_canonical_row(&malformed).is_err());

    let mut malformed = envelope.clone();
    malformed[RECOVERY_MAGIC.len()] = 0xff;
    assert!(decode_package_review_canonical_row(&malformed).is_err());

    let mut stale = envelope.clone();
    stale[RECOVERY_MAGIC.len()..RECOVERY_MAGIC.len() + 2].copy_from_slice(&7u16.to_le_bytes());
    assert!(decode_package_review_canonical_row(&stale).is_err());

    let canonical = canonical_range(&envelope);
    let offsets = canonical_offsets(&envelope[canonical.clone()]);

    let mut malformed = envelope.clone();
    malformed[canonical.start] ^= 0xff;
    assert!(decode_package_review_canonical_row(&malformed).is_err());

    let mut malformed = envelope.clone();
    malformed[canonical.start + ROW_MAGIC.len()] = 0xff;
    assert!(decode_package_review_canonical_row(&malformed).is_err());

    let mut malformed = envelope.clone();
    malformed[canonical.start + ROW_MAGIC.len() + 2] = 0xff;
    assert!(decode_package_review_canonical_row(&malformed).is_err());

    let mut malformed = envelope.clone();
    malformed[canonical.start + ROW_MAGIC.len() + 4..canonical.start + ROW_MAGIC.len() + 36]
        .fill(0);
    assert!(decode_package_review_canonical_row(&malformed).is_err());

    let mut malformed = envelope.clone();
    malformed[canonical.start + offsets.target_start] = 0xff;
    assert!(decode_package_review_canonical_row(&malformed).is_err());

    let mut malformed = envelope.clone();
    malformed[canonical.start + offsets.kind] = 0xff;
    assert!(decode_package_review_canonical_row(&malformed).is_err());

    let mut malformed = envelope.clone();
    malformed[canonical.start + offsets.risk] = 2;
    assert!(decode_package_review_canonical_row(&malformed).is_err());

    let mut malformed = envelope.clone();
    malformed[canonical.start + offsets.key_length..canonical.start + offsets.key_length + 8]
        .copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(decode_package_review_canonical_row(&malformed).is_err());

    let mut malformed = envelope.clone();
    malformed[canonical.start + offsets.value_length..canonical.start + offsets.value_length + 8]
        .copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(decode_package_review_canonical_row(&malformed).is_err());

    let source = source_offsets(&envelope, canonical.end);
    let mut malformed = envelope.clone();
    malformed[source.owner] = 0xff;
    assert!(decode_package_review_canonical_row(&malformed).is_err());

    let mut malformed = envelope.clone();
    malformed[source.path_start] = b'/';
    assert!(decode_package_review_canonical_row(&malformed).is_err());

    let mut malformed = envelope.clone();
    let start = malformed[source.start..source.start + 8].to_vec();
    malformed[source.end..source.end + 8].copy_from_slice(&start);
    assert!(decode_package_review_canonical_row(&malformed).is_err());

    let mut malformed = envelope.clone();
    malformed[source.role] = 0xff;
    assert!(decode_package_review_canonical_row(&malformed).is_err());

    let defaults = PackageReviewCanonicalRowRecoveryLimits::default();
    let too_small = PackageReviewCanonicalRowRecoveryLimits::new(
        envelope.len() - 1,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
    );
    assert!(decode_package_review_canonical_row_with_limits(&envelope, too_small).is_err());

    let too_small = PackageReviewCanonicalRowRecoveryLimits::new(
        usize::MAX,
        authored.canonical_bytes().len() - 1,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
    );
    assert!(decode_package_review_canonical_row_with_limits(&envelope, too_small).is_err());

    let too_small = PackageReviewCanonicalRowRecoveryLimits::new(
        usize::MAX,
        usize::MAX,
        usize::MAX,
        authored.key_bytes().len() - 1,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
    );
    assert!(decode_package_review_canonical_row_with_limits(&envelope, too_small).is_err());
    assert_ne!(defaults, too_small);

    let no_sources = PackageReviewCanonicalRowRecoveryLimits::new(
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        0,
        usize::MAX,
        usize::MAX,
        usize::MAX,
    );
    assert!(decode_package_review_canonical_row_with_limits(&envelope, no_sources).is_err());

    let tiny_envelope = PackageReviewCanonicalRowRecoveryLimits::new(
        envelope.len() - 1,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
    );
    assert!(
        omega_package_evidence::encoding::encode_package_review_canonical_row_with_limits(
            authored,
            tiny_envelope
        )
        .is_err()
    );
}

#[test]
fn decoder_rejects_duplicate_compiler_derivation_sidecars() {
    let Some((_target, rows)) = fixture_rows() else {
        return;
    };
    let derived = rows
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::ProjectionHeader)
        .expect("compiler-derived projection header");
    let mut envelope = encode_package_review_canonical_row(derived).expect("encode derived row");
    let canonical = canonical_range(&envelope);
    let source_count = canonical.end;
    assert_eq!(read_u64(&envelope, source_count), 0);
    let derivation_count = source_count + 8;
    assert_eq!(read_u64(&envelope, derivation_count), 1);
    envelope[derivation_count..derivation_count + 8].copy_from_slice(&2u64.to_le_bytes());
    let derivation = *envelope.last().expect("one derivation tag");
    envelope.push(derivation);
    assert!(decode_package_review_canonical_row(&envelope).is_err());

    let mut unknown = encode_package_review_canonical_row(derived).expect("encode derived row");
    *unknown.last_mut().expect("one derivation tag") = 0xff;
    assert!(decode_package_review_canonical_row(&unknown).is_err());
}

#[derive(Clone, Copy)]
struct CanonicalOffsets {
    target_start: usize,
    kind: usize,
    risk: usize,
    key_length: usize,
    value_length: usize,
}

fn canonical_range(envelope: &[u8]) -> std::ops::Range<usize> {
    let length_offset = RECOVERY_MAGIC.len() + 2;
    let start = length_offset + 8;
    let length = usize::try_from(read_u64(envelope, length_offset)).expect("canonical length");
    start..start + length
}

fn canonical_offsets(canonical: &[u8]) -> CanonicalOffsets {
    let target_length = ROW_MAGIC.len() + 2 + 2 + 32;
    let target_start = target_length + 8;
    let target_bytes = usize::try_from(read_u64(canonical, target_length)).expect("target length");
    let kind = target_start + target_bytes;
    let risk = kind + 1;
    let key_length = risk + 1;
    let key_bytes = usize::try_from(read_u64(canonical, key_length)).expect("row key length");
    CanonicalOffsets {
        target_start,
        kind,
        risk,
        key_length,
        value_length: key_length + 8 + key_bytes,
    }
}

#[derive(Clone, Copy)]
struct SourceOffsets {
    owner: usize,
    path_start: usize,
    start: usize,
    end: usize,
    role: usize,
}

fn source_offsets(envelope: &[u8], source_count: usize) -> SourceOffsets {
    assert_eq!(read_u64(envelope, source_count), 1);
    let owner = source_count + 8;
    let path_length = owner + 1 + 32;
    let path_start = path_length + 8;
    let path_bytes = usize::try_from(read_u64(envelope, path_length)).expect("source path length");
    let start = path_start + path_bytes;
    SourceOffsets {
        owner,
        path_start,
        start,
        end: start + 8,
        role: start + 16,
    }
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("eight-byte test frame"),
    )
}

fn host_target_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("windows_x86_64"),
        ("linux", "x86_64") => Some("linux_x86_64"),
        ("linux", "aarch64") => Some("linux_arm64"),
        ("macos", "aarch64") => Some("macos_arm64"),
        _ => None,
    }
}
