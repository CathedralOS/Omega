//! Trust report identity, validation, and rendering regressions.

use super::{TrustProviderRealization, TrustProviderRequirementRow, TrustReport};
use effects::provider_plan::{
    EvaluatedBindingEvaluationDigest, EvaluatedBindingMaterializationDigest,
    EvaluatedBindingProducerClosureDigest, EvaluatedBindingReceipt, EvaluatedBindingUsage,
    EvaluatedForeignImport,
};
use effects::{ForeignLocatorCandidate, NormalizedForeignLocator, normalize_foreign_locator};
use target::TargetProfile;

fn evaluated_import(locator: NormalizedForeignLocator, seed: u8) -> EvaluatedForeignImport {
    let usage = EvaluatedBindingUsage::from_evaluator(7, 1, 10, 1_000, 0, 0, 4, 12, 3, 0)
        .expect("valid fixture usage");
    let receipt = EvaluatedBindingReceipt::from_evaluation(
        None,
        format!("fixture::producer::{seed}"),
        EvaluatedBindingProducerClosureDigest::from_bytes([seed; 32]).unwrap(),
        1,
        usage,
        EvaluatedBindingEvaluationDigest::from_bytes([seed.wrapping_add(1); 32]).unwrap(),
        1,
        EvaluatedBindingMaterializationDigest::from_bytes([seed.wrapping_add(2); 32]).unwrap(),
        locator.identity_digest(),
    )
    .expect("valid fixture receipt");
    EvaluatedForeignImport::from_retained_evidence(locator, receipt)
        .expect("receipt matches fixture locator")
}

fn normalized_windows_import(library: &[u8], export: &[u8]) -> TrustProviderRealization {
    let locator = normalize_foreign_locator(
        ForeignLocatorCandidate::PeByName {
            library: library.to_vec(),
            export: export.to_vec(),
        },
        TargetProfile::WindowsX64,
    )
    .expect("valid normalized Windows import");
    TrustProviderRealization::Import {
        evaluated: evaluated_import(locator, 11),
    }
}

fn normalized_macos_import(install_name: &[u8], symbol: &[u8]) -> TrustProviderRealization {
    let locator = normalize_foreign_locator(
        ForeignLocatorCandidate::MachODylibSymbol {
            install_name: install_name.to_vec(),
            symbol: symbol.to_vec(),
        },
        TargetProfile::MacosArm64,
    )
    .expect("valid normalized Mach-O import");
    TrustProviderRealization::Import {
        evaluated: evaluated_import(locator, 21),
    }
}

fn test_provider_plan_digest() -> effects::provider_plan::ProviderPlanDigest {
    effects::provider_plan::ProviderPlan::default().identity_digest()
}

fn trust_provider_requirement(
    target: &str,
    realization: TrustProviderRealization,
) -> TrustProviderRequirementRow {
    TrustProviderRequirementRow {
        provider_plan: "ForeignProvider::satisfies::Foreign".to_owned(),
        provider_plan_report_fingerprint: 0x1234,
        provider_plan_digest: test_provider_plan_digest(),
        provider_type: String::new(),
        provider_type_package_identity: None,
        target: target.to_owned(),
        provider_origin_package_identity: None,
        provider_origin_package: String::new(),
        service_schema: "Foreign".to_owned(),
        service_schema_package_identity: None,
        calling_plan_report_fingerprint: None,
        calling_plan_commitment: None,
        selected: true,
        requirement_owner: "Foreign".to_owned(),
        requirement_owner_package_identity: None,
        requirement_identity: "Foreign::invoke".to_owned(),
        method: "invoke".to_owned(),
        parameter_type_identities: Vec::new(),
        result_type_identity: None,
        service_reach: Vec::new(),
        synchronous_invocations: Vec::new(),
        may_suspend: false,
        may_block: false,
        terminates_guarantee: false,
        termination_premises: Vec::new(),
        realization,
        provenance: "root grant (build.omg)".to_owned(),
        grant_selectors: vec!["Foreign".to_owned()],
        standing_warning: false,
    }
}

#[test]
fn trust_provider_realizations_distinguish_checked_and_opaque_leaves() {
    assert_eq!(
        TrustProviderRealization::CheckedAdapter {
            machine_identity: "ConsoleProvider::write".to_owned(),
            machine_package_identity: semantic_vocabulary::PackageKeyIdentity::from_digest(
                [0x5a; 32]
            ),
        }
        .report_text(),
        "checked adapter `ConsoleProvider::write`"
    );
    assert_eq!(
        TrustProviderRealization::Syscall { number: 60 }.report_text(),
        "syscall 60"
    );
}

#[test]
fn normalized_foreign_locator_mutations_change_trust_identity_and_exact_output() {
    let baseline = normalized_windows_import(b"opaque\xff.dll", b"invoke_raw");
    let changed_library = normalized_windows_import(b"opaque\xfe.dll", b"invoke_raw");
    let changed_export = normalized_windows_import(b"opaque\xff.dll", b"invoke_next");

    assert_ne!(
        baseline.foreign_locator_compatibility_report_identity(),
        changed_library.foreign_locator_compatibility_report_identity(),
    );
    assert_ne!(
        baseline.foreign_locator_compatibility_report_identity(),
        changed_export.foreign_locator_compatibility_report_identity(),
    );

    let text = baseline.report_text();
    let identity = baseline
        .foreign_locator_compatibility_report_identity()
        .expect("normalized import identity");
    assert!(text.contains(&format!("PeByName [{identity:016x}]")));
    assert!(text.contains("target `windows_x86_64`"));
    assert!(text.contains("library bytes 0x6f7061717565ff2e646c6c"));
    assert!(text.contains("export bytes 0x696e766f6b655f726177"));
    assert!(!text.contains("opaque.dll"));
}

#[test]
fn macho_locator_trust_report_keeps_raw_install_name_and_symbol() {
    let baseline = normalized_macos_import(b"/usr/lib/libSystem.B.dylib", b"_write\xff");
    assert_ne!(
        baseline.foreign_locator_compatibility_report_identity(),
        normalized_macos_import(b"/usr/lib/libobjc.A.dylib", b"_write\xff")
            .foreign_locator_compatibility_report_identity(),
    );
    assert_ne!(
        baseline.foreign_locator_compatibility_report_identity(),
        normalized_macos_import(b"/usr/lib/libSystem.B.dylib", b"_read\xff")
            .foreign_locator_compatibility_report_identity(),
    );

    let text = baseline.report_text();
    assert!(text.contains("MachODylibSymbol ["));
    assert!(text.contains("target `macos_arm64`"));
    assert!(
        text.contains("install-name bytes 0x2f7573722f6c69622f6c696253797374656d2e422e64796c6962")
    );
    assert!(text.contains("symbol bytes 0x5f7772697465ff"));
    assert!(!text.contains("_write"));
}

#[test]
fn trust_report_rejects_normalized_locator_under_a_different_target() {
    let report = TrustReport {
        provider_requirements: vec![trust_provider_requirement(
            "linux_x86_64",
            normalized_windows_import(b"opaque.dll", b"invoke_raw"),
        )],
        ..Default::default()
    };

    let diagnostic = report
        .validate()
        .expect_err("mismatched target must fail before artifact installation");
    assert!(diagnostic.message.contains("targets `windows_x86_64`"));
    assert!(diagnostic.message.contains("reports target `linux_x86_64`"));
}
