use declaration_contracts::source_identity::DependencyScope;
use source_admission::SourceFile;
use source_admission::SourceOrigin;
use source_admission::SourceResolutionStratum;
use source_text::SourceId;

const SOURCE: &str = "public machine fixture() {}\n";

#[test]
fn source_commitments_preserve_the_published_framing() {
    let file = SourceFile {
        source_id: SourceId(0),
        path: "/compiler/core/identity/fixture.omg".into(),
        package_root: "/compiler/core".into(),
        package_identity: None,
        dependency_scope: DependencyScope::Product,
        origin: SourceOrigin::Toolchain,
        resolution_stratum: SourceResolutionStratum::Base,
        source: SOURCE.into(),
    };
    assert_eq!(
        source_admission::source_identity::toolchain_source_identity_digest(&file).unwrap(),
        [
            244, 35, 32, 121, 111, 242, 75, 63, 193, 137, 182, 107, 227, 55, 85, 252, 89, 136, 244,
            45, 227, 216, 150, 148, 241, 120, 73, 59, 38, 136, 196, 72
        ],
    );
    assert_eq!(
        declaration_contracts::source_identity::toolchain_owned_identity_digest(
            "core",
            vec!["identity".into(), "fixture.omg".into()],
            SOURCE.as_bytes(),
        )
        .unwrap(),
        source_admission::source_identity::toolchain_source_identity_digest(&file).unwrap(),
    );
    assert_eq!(
        declaration_contracts::entry_contract_identity::digest(
            "pkg::entry",
            "entry.omg",
            SOURCE.as_bytes()
        ),
        [
            6, 170, 103, 126, 107, 2, 39, 225, 171, 188, 216, 213, 129, 189, 172, 64, 12, 211, 219,
            10, 111, 157, 9, 118, 169, 171, 139, 33, 211, 88, 68, 130
        ],
    );
}
