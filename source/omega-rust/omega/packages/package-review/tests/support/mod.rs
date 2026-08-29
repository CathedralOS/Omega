#![allow(dead_code, unused_imports)]

pub(crate) use omega_build_evaluation::BuildObservationClass;
pub(crate) use omega_compiler::{CheckedCompilation, compile_to_checked_with_packages};
pub(crate) use omega_package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
pub(crate) use omega_package_review::{
    CheckedPackageReviewProjection, PACKAGE_REVIEW_ENCODING_VERSION,
    PACKAGE_REVIEW_ROW_ENCODING_VERSION, PackageReviewArithmeticDomain,
    PackageReviewByteSequencePredicate, PackageReviewCallableRole, PackageReviewCallableSupply,
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk, PackageReviewCastForm,
    PackageReviewCheckedServiceReach, PackageReviewCompilerIntrinsicExecution,
    PackageReviewConformanceSubject, PackageReviewContractBinaryOperator,
    PackageReviewContractExpression, PackageReviewContractFact, PackageReviewContractKind,
    PackageReviewContractOperatorMeaning, PackageReviewContractStaticArgument,
    PackageReviewContractUnaryOperator, PackageReviewCrashInterface, PackageReviewCrashRouteGuard,
    PackageReviewDangerousAuthorityClass, PackageReviewDataKind, PackageReviewDataMember,
    PackageReviewDomainAliasAtom, PackageReviewDomainClassification,
    PackageReviewDomainEstablishmentKind, PackageReviewDomainSemanticRole,
    PackageReviewExternalBinding, PackageReviewExternalRequirement, PackageReviewFloatLiteral,
    PackageReviewMachineParameterContract, PackageReviewNominalOwner,
    PackageReviewPropositionBinderKind, PackageReviewPropositionBinderValue,
    PackageReviewPropositionEvidence, PackageReviewPublicPropositionBody,
    PackageReviewRepresentationAbiCommitment, PackageReviewRepresentationMechanism,
    PackageReviewSemanticDependencyExposure, PackageReviewSemanticDependencyKind,
    PackageReviewSourceLocationOwner, PackageReviewSourceLocationRole,
    PackageReviewSynchronousInvocation, PackageReviewSyntheticSourceKind,
    PackageReviewTypeParameterKind, decode_ordinary_package_obligation_ledger,
    decode_package_review_canonical_row, encode_ordinary_package_obligation_ledger,
    encode_package_review_canonical_row, ordinary_package_obligation_ledger_fingerprint,
    ordinary_package_obligation_ledger_from_compiler_rows, project_checked_package_review,
    recover_ordinary_package_obligation_ledger, validate_ordinary_package_obligation_ledger,
};
pub(crate) use psi_core::PackageKeyIdentity;
pub(crate) use std::fs;
pub(crate) use std::path::{Path, PathBuf};
pub(crate) use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
pub(crate) const LEDGER_MAGIC: &[u8] = b"OMEGA-ORDINARY-PACKAGE-OBLIGATION-LEDGER\0";

pub(crate) fn read_ledger_u64(bytes: &[u8], position: &mut usize) -> usize {
    let end = *position + 8;
    let value = u64::from_le_bytes(bytes[*position..end].try_into().unwrap());
    *position = end;
    usize::try_from(value).unwrap()
}

pub(crate) fn ledger_target_range(bytes: &[u8]) -> std::ops::Range<usize> {
    let mut position = LEDGER_MAGIC.len() + 4 * std::mem::size_of::<u16>() + 32;
    let length = read_ledger_u64(bytes, &mut position);
    position..position + length
}

pub(crate) fn ledger_closure_package_range(bytes: &[u8]) -> std::ops::Range<usize> {
    let target = ledger_target_range(bytes);
    let mut position = target.end + 32;
    let count = read_ledger_u64(bytes, &mut position);
    position..position + count * 32
}

pub(crate) fn ledger_row_frames(bytes: &[u8]) -> Vec<std::ops::Range<usize>> {
    let packages = ledger_closure_package_range(bytes);
    let mut position = packages.end;
    let dependencies = read_ledger_u64(bytes, &mut position);
    for _ in 0..dependencies {
        position += 32;
        let alias_length = read_ledger_u64(bytes, &mut position);
        position += alias_length + 32;
    }
    let rows = read_ledger_u64(bytes, &mut position);
    (0..rows)
        .map(|_| {
            let start = position;
            let length = read_ledger_u64(bytes, &mut position);
            position += length;
            start..position
        })
        .collect()
}

pub(crate) struct TempPackage(pub(crate) PathBuf);

impl TempPackage {
    pub(crate) fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-package-review-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create package review fixture");
        Self(path)
    }

    pub(crate) fn write(&self, path: impl AsRef<Path>, source: &str) {
        fs::write(self.0.join(path), source).expect("write package review fixture source");
    }
}

impl Drop for TempPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(crate) fn package_identity() -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([41; 32]).expect("nonzero package identity")
}

pub(crate) fn package_inputs(root: &Path) -> PackageCompilationInputs {
    PackageCompilationInputs::new(
        package_identity(),
        vec![PackageSourceBinding::new(
            package_identity(),
            "review-fixture",
            root.to_owned(),
        )],
        Vec::new(),
    )
    .expect("single-package review graph should validate")
}

pub(crate) fn public_quotient_source(
    carrier: &str,
    relation: &str,
    evidence: &str,
    reverse_relation: bool,
) -> String {
    let (
        relation_body,
        symmetric_requires,
        symmetric_ensures,
        transitive_requires,
        transitive_ensures,
    ) = if reverse_relation {
        ("b == a", "b == a", "a == b", "b == a\n    c == b", "c == a")
    } else {
        ("a == b", "a == b", "b == a", "a == b\n    b == c", "a == c")
    };
    format!(
        r#"use omega::language::core::relation;

pub data {carrier} {{
    case Zero;
    case Next(previous: {carrier});
}}

pub proposition {relation}(a: {carrier}, b: {carrier}) = {relation_body};

machine equivalent_reflexive(a: {carrier})
ensures a == a
{{
}}

machine equivalent_symmetric(a: {carrier}, b: {carrier})
requires {symmetric_requires}
ensures {symmetric_ensures}
{{
}}

machine equivalent_transitive(a: {carrier}, b: {carrier}, c: {carrier})
requires
    {transitive_requires}
ensures {transitive_ensures}
{{
}}

{evidence}: satisfies Equivalence<{carrier}, {relation}> {{
    Reflexive::reflexive = equivalent_reflexive;
    Symmetric::symmetric = equivalent_symmetric;
    Transitive::transitive = equivalent_transitive;
}}

pub data EquivalenceClass = {carrier} % {relation}
where {relation} satisfies
    Equivalence<{carrier}, {relation}>
    as {evidence};
"#,
    )
}

pub(crate) fn host_target_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("windows_x64"),
        ("linux", "x86_64") => Some("linux_x64"),
        ("linux", "aarch64") => Some("linux_arm64"),
        ("macos", "aarch64") => Some("macos_arm64"),
        _ => None,
    }
}
