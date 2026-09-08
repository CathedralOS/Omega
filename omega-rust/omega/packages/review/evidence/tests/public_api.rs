mod support;

#[path = "public_api/data_and_quotients.rs"]
mod data_and_quotients;
#[path = "public_api/data_invariants.rs"]
mod data_invariants;
#[path = "public_api/domain_predicates.rs"]
mod domain_predicates;
#[path = "public_api/module_constants.rs"]
mod module_constants;
#[path = "public_api/module_namespaces.rs"]
mod module_namespaces;
#[path = "public_api/public_domains.rs"]
mod public_domains;
#[path = "public_api/traits_and_lifetimes.rs"]
mod traits_and_lifetimes;

#[test]
fn module_constant_domain_index_enters_canonical_public_data_artifact() {
    use support::*;
    use typed_trees::data::DataMember;
    use typed_trees::types::{TypeConstraintNode, TypeReferenceNode};

    let project = |package: &TempPackage, size: u64, argument: &str| {
        package.write(
            "build.omg",
            "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }",
        );
        package.write(
            "combat.omg",
            &format!("module combat; pub const SIZE: u64 = {size};"),
        );
        package.write("rooms.omg", "module rooms; pub const SIZE: u64 = 9;");
        package.write(
            "main.omg",
            &format!(
                "use combat; use rooms; const SIZE: u64 = 1;
             pub domain<T, const N: u64> T::Indexed<N>;
             pub data Root {{ value: u64 in Indexed<{argument}>; }}"
            ),
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x86_64"),
            package_inputs(&package.0),
        )
        .expect("public indexed domain application checks");
        let [family] = checked.typed.domain_definitions() else {
            panic!("one authored indexed family");
        };
        let root = checked
            .typed
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Root")
            .expect("Root declaration");
        let [DataMember::Field(field)] = checked.typed.data_members(root) else {
            panic!("one public constrained field");
        };
        let TypeReferenceNode::Constrained { constraints, .. } = checked
            .typed
            .type_reference_table
            .type_reference(field.type_reference)
        else {
            panic!("domain constraint remains in semantic API input");
        };
        let [TypeConstraintNode::Domain(domain)] =
            checked.typed.type_reference_table.constraints(*constraints)
        else {
            panic!("one exact domain application");
        };
        assert_eq!(
            domain.symbol, family.symbol,
            "canonical domain refers to its declaration, not its rendered spelling"
        );
        assert!(domain.symbol.is_valid());
        assert!(domain.semantic_id.is_valid());
        assert_eq!(domain.arguments.len(), 1);
        project_checked_package_review(&checked).expect("indexed domain API projects")
    };
    let first_package = TempPackage::new();
    let relocated_package = TempPackage::new();
    let original = project(&first_package, 2, "combat::SIZE + 1");
    let relocated = project(&relocated_package, 2, "combat::SIZE + 1");
    assert_ne!(first_package.0, relocated_package.0);
    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        relocated.canonical_review_bytes().unwrap()
    );
    let data_rows = |review: &CheckedPackageReviewProjection| {
        review
            .canonical_rows()
            .unwrap()
            .into_iter()
            .filter(|row| row.kind() == PackageReviewCanonicalRowKind::PublicData)
            .collect::<Vec<_>>()
    };
    let rows = data_rows(&original);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows, data_rows(&relocated));
    for row in &rows {
        let bytes = encode_package_review_canonical_row(row).unwrap();
        let decoded = decode_package_review_canonical_row(&bytes).unwrap();
        assert_eq!(decoded.key_bytes(), row.key_bytes());
        assert_eq!(decoded.canonical_bytes(), row.canonical_bytes());
    }
    let payloads = |review: &CheckedPackageReviewProjection| {
        data_rows(review)
            .iter()
            .map(|row| (row.key_bytes().to_vec(), row.canonical_bytes().to_vec()))
            .collect::<Vec<_>>()
    };
    let literal = project(&TempPackage::new(), 2, "3");
    let equivalent = project(&TempPackage::new(), 2, "combat::SIZE + combat::SIZE - 1");
    assert_eq!(payloads(&original), payloads(&literal));
    assert_eq!(payloads(&original), payloads(&equivalent));
    let changed = project(&TempPackage::new(), 3, "combat::SIZE + 1");
    let changed_literal = project(&TempPackage::new(), 3, "4");
    assert_eq!(payloads(&changed), payloads(&changed_literal));
    assert_ne!(payloads(&original), payloads(&changed));
}

#[test]
fn module_constant_index_enters_canonical_public_data_artifact() {
    use support::*;

    let project = |package: &TempPackage, combat_size: u64, argument: &str| {
        package.write(
            "build.omg",
            "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }",
        );
        package.write(
            "combat.omg",
            &format!("module combat; pub const SIZE: u64 = {combat_size};"),
        );
        package.write("rooms.omg", "module rooms; pub const SIZE: u64 = 9;");
        package.write(
            "main.omg",
            &format!(
                "use combat; use rooms; const SIZE: u64 = 1;
             pub data Buffer<const N: u64> {{ value: [u8; N]; }}
             pub data Root {{ value: Buffer<{argument}>; }}"
            ),
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x86_64"),
            package_inputs(&package.0),
        )
        .expect("public named constant application checks");
        project_checked_package_review(&checked).expect("public constant application projects")
    };
    let original_package = TempPackage::new();
    let relocated_package = TempPackage::new();
    assert_ne!(original_package.0, relocated_package.0);
    let original = project(&original_package, 2, "combat::SIZE");
    let relocated = project(&relocated_package, 2, "combat::SIZE");
    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        relocated.canonical_review_bytes().unwrap()
    );

    let data_rows = |review: &CheckedPackageReviewProjection| {
        review
            .canonical_rows()
            .unwrap()
            .into_iter()
            .filter(|row| row.kind() == PackageReviewCanonicalRowKind::PublicData)
            .collect::<Vec<_>>()
    };
    let original_rows = data_rows(&original);
    let relocated_rows = data_rows(&relocated);
    assert!(!original_rows.is_empty());
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

    // Compare serialized public-data payloads against literal applications;
    // a generated display name alone cannot establish the selected index.
    let payloads = |review: &CheckedPackageReviewProjection| {
        data_rows(review)
            .iter()
            .map(|row| (row.key_bytes().to_vec(), row.canonical_bytes().to_vec()))
            .collect::<Vec<_>>()
    };
    let literal_two = project(&TempPackage::new(), 2, "2");
    assert_eq!(payloads(&original), payloads(&literal_two));
    let changed = project(&original_package, 3, "combat::SIZE");
    let literal_three = project(&TempPackage::new(), 3, "3");
    assert_eq!(payloads(&changed), payloads(&literal_three));
    assert_ne!(payloads(&original), payloads(&changed));
    let root_shape = |review: &CheckedPackageReviewProjection| {
        review
            .public_data()
            .iter()
            .find(|shape| shape.identity().path() == "Root")
            .cloned()
            .expect("Root retains a canonical public data row")
    };
    assert_eq!(
        root_shape(&original).identity(),
        root_shape(&changed).identity()
    );
    assert_ne!(
        root_shape(&original).members(),
        root_shape(&changed).members()
    );

    let compound = project(&original_package, 2, "combat::SIZE + 1");
    let relocated_compound = project(&relocated_package, 2, "combat::SIZE + 1");
    assert_eq!(
        compound.canonical_review_bytes().unwrap(),
        relocated_compound.canonical_review_bytes().unwrap()
    );
    assert_eq!(data_rows(&compound), data_rows(&relocated_compound));
    let compound_literal = project(&TempPackage::new(), 2, "3");
    assert_eq!(payloads(&compound), payloads(&compound_literal));
    let equivalent_compound = project(&TempPackage::new(), 2, "combat::SIZE + combat::SIZE - 1");
    assert_eq!(payloads(&compound), payloads(&equivalent_compound));
    let changed_compound = project(&TempPackage::new(), 3, "combat::SIZE + 1");
    let changed_literal = project(&TempPackage::new(), 3, "4");
    assert_eq!(payloads(&changed_compound), payloads(&changed_literal));
    assert_ne!(payloads(&compound), payloads(&changed_compound));
}
