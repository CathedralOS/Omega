use super::*;

pub(super) const ALL_FAMILIES: &str = r#"pub data Token { value: i64; }
pub trait Marker {}
pub trait Alternate {}
pub trait Protocol { machine probe(value: i64) -> i64; }
pub Primary: Token satisfies Marker {}
pub proposition ready() = true;
pub const LIMIT: u64 = 4;
pub domain Token::Nonnegative requires self.value >= 0;
pub operator Token::identity(value: i64) -> i64;
pub machine inspect(value: i64) -> i64 { helper(value) }
machine helper(value: i64) -> i64 { value }
"#;

#[test]
fn private_semantic_consumers_are_grouped_without_losing_nominal_dependencies() {
    let source = r#"
pub data Token { value: i64; }
pub machine inspect(value: Token) -> Token { helper(value) }
machine helper(value: Token) -> Token { value }
"#;
    let original = project(&Fixture::local(source));
    let renamed = project(&Fixture::local(&source.replace("helper", "implementation")));
    assert!(original.semantic_dependencies().iter().any(|dependency| {
        matches!(
            dependency.consumer(),
            PackagePolicySemanticDependencyConsumer::PackageImplementation
        ) && dependency.dependency().path() == "Token"
            && dependency.exposure()
                == PackageReviewSemanticDependencyExposure::PrivateImplementation
    }));
    assert_eq!(
        original.semantic_dependencies(),
        renamed.semantic_dependencies()
    );
    assert_eq!(
        original.canonical_bytes().unwrap(),
        renamed.canonical_bytes().unwrap()
    );
}

#[test]
fn all_seven_public_families_survive_complete_canonical_recovery() {
    let policy = project(&Fixture::local(ALL_FAMILIES));
    assert_eq!(policy.public_traits().len(), 3);
    assert_eq!(policy.public_conformances().len(), 1);
    assert_eq!(policy.public_domains().len(), 1);
    assert_eq!(policy.public_propositions().len(), 1);
    assert_eq!(policy.public_consts().len(), 1);
    assert_eq!(policy.public_operators().len(), 1);
    assert_eq!(policy.public_data().len(), 1);
    let identities = policy
        .public_traits()
        .iter()
        .map(|value| value.identity())
        .chain(
            policy
                .public_conformances()
                .iter()
                .map(|value| value.identity()),
        )
        .chain(policy.public_domains().iter().map(|value| value.identity()))
        .chain(
            policy
                .public_propositions()
                .iter()
                .map(|value| value.identity()),
        )
        .chain(policy.public_consts().iter().map(|value| value.identity()))
        .chain(
            policy
                .public_operators()
                .iter()
                .map(|value| value.coordinate().identity()),
        )
        .chain(policy.public_data().iter().map(|value| value.identity()));
    for identity in identities {
        assert_eq!(
            identity.owner(),
            PackageReviewNominalOwner::Package(package_identity())
        );
    }
    assert!(
        policy
            .callables()
            .callables()
            .iter()
            .any(|row| row.role() == PackagePolicyCallableRole::Public)
    );
    assert!(
        policy
            .callables()
            .callables()
            .iter()
            .any(|row| row.role() == PackagePolicyCallableRole::Build)
    );
}

#[test]
fn every_public_family_changes_the_baseline_when_its_meaning_changes() {
    let original = project(&Fixture::local(ALL_FAMILIES));
    let bytes = original.canonical_bytes().unwrap();
    for (family, before, after) in [
        (
            "trait",
            "probe(value: i64) -> i64",
            "probe(value: i64) -> i32",
        ),
        (
            "conformance",
            "Token satisfies Marker",
            "Token satisfies Alternate",
        ),
        ("domain", "self.value >= 0", "self.value >= 1"),
        ("proposition", "ready() = true", "ready() = false"),
        ("const", "LIMIT: u64 = 4", "LIMIT: u64 = 5"),
        (
            "operator",
            "identity(value: i64) -> i64",
            "identity(value: i64) -> i32",
        ),
        (
            "data",
            "Token { value: i64; }",
            "Token { value: i64; tag: bool; }",
        ),
    ] {
        assert_eq!(ALL_FAMILIES.matches(before).count(), 1);
        let changed = project(&Fixture::local(&ALL_FAMILIES.replace(before, after)));
        let family_changed = match family {
            "trait" => original.public_traits() != changed.public_traits(),
            "conformance" => original.public_conformances() != changed.public_conformances(),
            "domain" => original.public_domains() != changed.public_domains(),
            "proposition" => original.public_propositions() != changed.public_propositions(),
            "const" => original.public_consts() != changed.public_consts(),
            "operator" => original.public_operators() != changed.public_operators(),
            "data" => original.public_data() != changed.public_data(),
            _ => unreachable!(),
        };
        assert!(family_changed, "{family} meaning must remain visible");
        assert_ne!(bytes, changed.canonical_bytes().unwrap(), "{family} drift");
    }
}

#[test]
fn source_locations_private_helpers_and_unreachable_private_declarations_are_not_baseline_meaning()
{
    let original = project(&Fixture::local(ALL_FAMILIES));
    let changed = project(&Fixture::local(&format!(
        "// Source moved without changing public policy.\n\n{}\nconst PRIVATE_LIMIT: u64 = 99;\nmachine unused() {{}}\n",
        ALL_FAMILIES.replace("helper", "implementation")
    )));
    assert_eq!(original, changed);
    assert_eq!(
        original.canonical_bytes().unwrap(),
        changed.canonical_bytes().unwrap()
    );
}

#[test]
fn public_type_const_and_lifetime_binder_renaming_preserves_complete_policy() {
    let source = r#"pub data View<'left, 'right> { first: &'left [u8]; second: &'right [u8]; }
pub trait Parent<'source, Element> { machine borrow(value: &'source Element) -> &'source Element; }
pub trait Child<'child, Item>: Parent<'child, Item> {}
pub proposition equivalent<Value>(left: Value, right: Value);
pub data Unit { code: u32; }
pub domain<Carrier, const Index: Unit> Carrier::Tagged<Index>;
pub machine keep<'input, const Width: u64>(value: &'input [u8; Width]) -> &'input [u8; Width] { value }
"#;
    let original = project(&Fixture::local(source));
    let renamed_source = source
        .replace("'left", "'primary")
        .replace("'right", "'secondary")
        .replace("'source", "'origin")
        .replace("Element", "Compared")
        .replace("'child", "'region")
        .replace("Item", "Selected")
        .replace("Value", "Operand")
        .replace("Carrier", "Subject")
        .replace("Index", "Tag")
        .replace("'input", "'borrow")
        .replace("Width", "Length");
    let renamed = project(&Fixture::local(&renamed_source));
    assert_eq!(original, renamed);
    assert_eq!(
        original.canonical_bytes().unwrap(),
        renamed.canonical_bytes().unwrap()
    );
    let changed = project(&Fixture::local(
        &source.replace("second: &'right", "second: &'left"),
    ));
    assert_ne!(original.public_data(), changed.public_data());
    assert_ne!(
        original.canonical_bytes().unwrap(),
        changed.canonical_bytes().unwrap()
    );
}

#[test]
fn same_spelled_foreign_declaration_owners_remain_distinct_without_importing_their_surface() {
    let root = "use dependency::api;\npub data Wrapper { value: Carrier; }\npub trait Protocol { machine exchange(value: Carrier) -> Carrier; }\n";
    let dependency = "pub data Carrier { value: u64; }\n";
    let first = project(&Fixture::foreign(
        root,
        dependency,
        PackageKeyIdentity::from_digest([42; 32]).unwrap(),
    ));
    let second = project(&Fixture::foreign(
        root,
        dependency,
        PackageKeyIdentity::from_digest([43; 32]).unwrap(),
    ));
    assert_eq!(
        first.public_data().len(),
        1,
        "foreign Carrier is a reference, not root public data"
    );
    assert_eq!(
        first.public_data()[0].identity(),
        second.public_data()[0].identity()
    );
    assert_ne!(first.public_data(), second.public_data());
    assert_ne!(first.public_traits(), second.public_traits());
    assert_ne!(
        first.canonical_bytes().unwrap(),
        second.canonical_bytes().unwrap()
    );
}
