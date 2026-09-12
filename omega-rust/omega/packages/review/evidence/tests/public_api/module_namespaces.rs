use crate::support::*;
use compiler::CheckedCompileRequest;

#[test]
fn module_nominals_have_distinct_canonical_rows_across_package_relocation() {
    let project = |package: &TempPackage| {
        package.write(
            "build.omg",
            "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }",
        );
        package.write("main.omg", "use combat; use rooms;");
        for module in ["combat", "rooms"] {
            package.write(
                format!("{module}.omg"),
                &format!("module {module}; pub data Point {{ value: u64; }}"),
            );
        }
        let checked = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
        })
        .expect("same-leaf module declarations check in one managed package");
        project_checked_package_review(&checked).expect("capture module nominal identities")
    };
    let first = TempPackage::new();
    let relocated = TempPackage::new();
    assert_ne!(first.0, relocated.0);
    let original = project(&first);
    let moved = project(&relocated);
    let mut paths = original
        .public_data()
        .iter()
        .map(|shape| {
            assert_eq!(
                shape.identity().owner(),
                PackageReviewNominalOwner::Package(package_identity()),
            );
            shape.identity().path()
        })
        .collect::<Vec<_>>();
    paths.sort_unstable();
    assert_eq!(paths, ["combat::Point", "rooms::Point"]);
    assert_eq!(original.public_data(), moved.public_data());

    let public_rows = |review: &CheckedPackageReviewProjection| {
        review
            .canonical_rows()
            .expect("capture canonical module rows")
            .into_iter()
            .filter(|row| row.kind() == PackageReviewCanonicalRowKind::PublicData)
            .collect::<Vec<_>>()
    };
    let original_rows = public_rows(&original);
    let moved_rows = public_rows(&moved);
    assert_eq!(original_rows.len(), 2);
    assert_ne!(original_rows[0].key_bytes(), original_rows[1].key_bytes());
    assert_ne!(
        original_rows[0].canonical_bytes(),
        original_rows[1].canonical_bytes()
    );
    assert_eq!(original_rows, moved_rows);
    for (original_row, moved_row) in original_rows.iter().zip(&moved_rows) {
        let bytes = encode_package_review_canonical_row(original_row).expect("encode module row");
        assert_eq!(
            bytes,
            encode_package_review_canonical_row(moved_row).expect("encode relocated module row"),
        );
        let recovered = decode_package_review_canonical_row(&bytes).expect("decode module row");
        assert_eq!(recovered.canonical_bytes(), original_row.canonical_bytes());
    }
}
