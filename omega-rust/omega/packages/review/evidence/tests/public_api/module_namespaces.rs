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
        let checked = compile_review_fixture(CheckedCompileRequest {
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

#[test]
fn module_issuer_routes_retain_exact_callable_namespaces() {
    let package = TempPackage::new();
    package.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }",
    );
    package.write("main.omg", "use alpha; use bravo;");
    for (module, domain) in [("alpha", "AlphaIssued"), ("bravo", "BravoIssued")] {
        package.write(
            format!("{module}.omg"),
            &format!(
                "module {module};\npub domain u64::{domain} requires self > 0; established by issue;\n\
                 pub machine issue() -> u64 in {domain} {{ 7 }}\n\
                 pub machine forward() -> u64 in {domain} {{ issue() }}\n"
            ),
        );
    }
    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
    })
    .expect("public issuers in distinct modules should check");
    let identities = ["alpha::issue", "bravo::issue"].map(|path| {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| checked.symbols.display_path(machine.symbol, "::") == path)
            .expect("exact module-owned issuer");
        let identity = checked
            .normalized_machine_overload_identity(machine)
            .expect("issuer entry signature");
        assert_eq!(identity.path(), path);
        assert_eq!(
            checked
                .machine_by_normalized_overload_identity(&identity.identity())
                .expect("normalized issuer identity resolves uniquely")
                .symbol,
            machine.symbol,
        );
        identity.identity()
    });
    assert_ne!(identities[0], identities[1]);
    let review = project_checked_package_review(&checked).expect("module issuer review");
    let routes = ["alpha::u64::AlphaIssued", "bravo::u64::BravoIssued"].map(|path| {
        let domain = review
            .public_domains()
            .iter()
            .find(|domain| domain.identity().path() == path)
            .expect("module-owned domain");
        let [route] = domain.establishment_routes() else {
            panic!("one public issuer")
        };
        route.machine_identity().expect("machine issuer").clone()
    });
    assert_eq!(routes[0].owner(), routes[1].owner());
    assert_eq!(routes[0].path(), identities[0]);
    assert_eq!(routes[1].path(), identities[1]);
    assert_ne!(routes[0], routes[1]);
    // Equal-length module names let us substitute only the exact callable
    // payload without modifying any wire lengths or framing.
    assert_eq!(identities[0].len(), identities[1].len());
    let rows = review
        .canonical_rows()
        .expect("canonical module issuer rows");
    let mut substitutions = 0;
    let supplied_rows = rows
        .iter()
        .map(|row| {
            let mut bytes = encode_package_review_canonical_row(row).unwrap();
            if row.kind() == PackageReviewCanonicalRowKind::PublicDomain {
                let positions = bytes
                    .windows(identities[0].len())
                    .enumerate()
                    .filter_map(|(index, value)| {
                        (value == identities[0].as_bytes()).then_some(index)
                    })
                    .collect::<Vec<_>>();
                for position in positions {
                    bytes[position..position + identities[0].len()]
                        .copy_from_slice(identities[1].as_bytes());
                    substitutions += 1;
                }
            }
            decode_package_review_canonical_row(&bytes).expect("inert substituted issuer row")
        })
        .collect::<Vec<_>>();
    assert_eq!(substitutions, 1);
    let supplied = recover_ordinary_package_obligation_ledger(
        checked.custody.dependency_closure().cloned().unwrap(),
        &supplied_rows,
    )
    .expect("substituted issuer ledger is structurally valid");
    assert!(validate_ordinary_package_obligation_ledger(&supplied, &checked).is_err());
}
