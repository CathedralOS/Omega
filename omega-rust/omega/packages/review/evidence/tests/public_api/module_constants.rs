use crate::support::*;

fn project(package: &TempPackage, combat_damage: u64) -> CheckedPackageReviewProjection {
    package.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }",
    );
    package.write("main.omg", "use combat; use rooms;");
    for (module, value) in [("combat", combat_damage), ("rooms", 9)] {
        package.write(
            format!("{module}.omg"),
            &format!("module {module}; pub const DAMAGE: u64 = {value};"),
        );
    }
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x86_64"),
        package_inputs(&package.0),
    )
    .expect("same-leaf module constants check in one managed package");
    project_checked_package_review(&checked).expect("capture exact public constant identities")
}

#[test]
fn module_constants_bind_canonical_paths_values_and_relocation_independent_bytes() {
    let original_package = TempPackage::new();
    let relocated_package = TempPackage::new();
    assert_ne!(original_package.0, relocated_package.0);
    let original = project(&original_package, 7);
    let relocated = project(&relocated_package, 7);
    let constants = original.public_consts();
    assert_eq!(constants.len(), 2);
    for (constant, (path, value_encoding)) in constants.iter().zip([
        ("combat::DAMAGE", "integer3:u641:7"),
        ("rooms::DAMAGE", "integer3:u641:9"),
    ]) {
        assert_eq!(constant.identity().path(), path);
        assert_eq!(
            constant.identity().owner(),
            PackageReviewNominalOwner::Package(package_identity())
        );
        assert_eq!(constant.canonical_value_encoding(), value_encoding);
    }
    assert_eq!(constants, relocated.public_consts());

    let rows = |review: &CheckedPackageReviewProjection| {
        review
            .canonical_rows()
            .expect("canonical constant rows")
            .into_iter()
            .filter(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConst)
            .collect::<Vec<_>>()
    };
    let original_rows = rows(&original);
    let relocated_rows = rows(&relocated);
    assert_eq!(original_rows.len(), 2);
    assert_ne!(original_rows[0].key_bytes(), original_rows[1].key_bytes());
    assert_eq!(original_rows, relocated_rows);
    for (original_row, relocated_row) in original_rows.iter().zip(&relocated_rows) {
        let encoded = encode_package_review_canonical_row(original_row).unwrap();
        assert_eq!(
            encoded,
            encode_package_review_canonical_row(relocated_row).unwrap()
        );
        let decoded = decode_package_review_canonical_row(&encoded).unwrap();
        assert_eq!(decoded.key_bytes(), original_row.key_bytes());
        assert_eq!(decoded.canonical_bytes(), original_row.canonical_bytes());
    }

    // The serializer keys by nominal identity but includes the exact constant
    // encoding in its payload. A value edit must not disappear as display text.
    let changed = project(&original_package, 8);
    let changed_rows = rows(&changed);
    assert_eq!(original_rows.len(), changed_rows.len());
    let mut changed_payloads = 0;
    for original_row in &original_rows {
        let changed_row = changed_rows
            .iter()
            .find(|row| row.key_bytes() == original_row.key_bytes())
            .expect("value change retains its constant coordinate");
        if changed_row.canonical_bytes() != original_row.canonical_bytes() {
            changed_payloads += 1;
        }
    }
    assert_eq!(changed_payloads, 1, "only combat's exact value changed");
}

#[test]
fn public_float_identity_retains_format_bits_and_exact_package_owner() {
    let package = TempPackage::new();
    package.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }",
    );
    package.write("main.omg", "use settings;");
    let project_float = |carrier: &str, literal: &str| {
        package.write(
            "settings.omg",
            &format!("module settings; pub const SCALE: {carrier} = {literal};"),
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x86_64"),
            package_inputs(&package.0),
        )
        .expect("public floating declaration checks");
        project_checked_package_review(&checked).expect("floating public API capture")
    };
    let original = project_float("f32", "1.5");
    let constant = &original.public_consts()[0];
    assert_eq!(constant.identity().path(), "settings::SCALE");
    assert_eq!(
        constant.identity().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    let row = |projection: &CheckedPackageReviewProjection| {
        projection
            .canonical_rows()
            .expect("public rows")
            .into_iter()
            .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConst)
            .expect("exact constant row")
    };
    let original_row = row(&original);
    assert_eq!(original_row, row(&project_float("f32", "1.500")));
    assert_eq!(
        row(&project_float("f32", "1e40")),
        row(&project_float("f32", "1e9999"))
    );
    for changed in [
        project_float("f32", "2.5"),
        project_float("f64", "1.5"),
        project_float("f32", "1e9999"),
        project_float("f32", "-1e9999"),
        project_float("f64", "1e9999"),
        project_float("f64", "-1e9999"),
    ] {
        let changed_row = row(&changed);
        assert_eq!(original_row.key_bytes(), changed_row.key_bytes());
        assert_ne!(
            original_row.canonical_bytes(),
            changed_row.canonical_bytes()
        );
        let encoded = encode_package_review_canonical_row(&changed_row)
            .expect("encode changed float declaration");
        let decoded = decode_package_review_canonical_row(&encoded)
            .expect("recover changed float declaration");
        assert_eq!(decoded.canonical_bytes(), changed_row.canonical_bytes());
    }
    let encoded =
        encode_package_review_canonical_row(&original_row).expect("encode float declaration");
    let decoded = decode_package_review_canonical_row(&encoded).expect("recover float declaration");
    assert_eq!(decoded.canonical_bytes(), original_row.canonical_bytes());
}
