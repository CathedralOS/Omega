use super::*;

#[test]
fn trait_and_nested_static_crash_guards_keep_same_spelled_foreign_helper_owners() {
    let helper = "pub machine permitted(flag: bool) -> bool terminates; { flag }\n";
    for declaration in [
        "pub trait Worker { machine run(flag: bool) crashes Trap permitted(flag); }\n",
        "pub trait Worker { machine register<machine Work>() where machine Work(flag: bool) crashes Trap permitted(flag); ; }\n",
    ] {
        let source = format!("use dependency::api;\n{declaration}");
        let first = project(&Fixture::foreign(
            &source,
            helper,
            PackageKeyIdentity::from_digest([42; 32]).unwrap(),
        ));
        let second = project(&Fixture::foreign(
            &source,
            helper,
            PackageKeyIdentity::from_digest([43; 32]).unwrap(),
        ));
        assert_eq!(first.public_traits().len(), 1);
        assert_eq!(
            first.public_traits()[0].identity(),
            second.public_traits()[0].identity()
        );
        assert_ne!(
            first.public_traits(),
            second.public_traits(),
            "foreign guard target must remain exact in {declaration}"
        );
        assert_ne!(
            first.canonical_bytes().unwrap(),
            second.canonical_bytes().unwrap()
        );
    }
}

#[test]
fn absent_requirement_result_and_explicit_empty_data_result_are_distinct() {
    let source = "pub data Unit {}\npub trait Worker { machine run(); }\n";
    let absent = project(&Fixture::local(source));
    let explicit = project(&Fixture::local(&source.replace("run()", "run() -> Unit")));
    assert_eq!(absent.public_data(), explicit.public_data());
    assert_ne!(absent.public_traits(), explicit.public_traits());
    assert_ne!(
        absent.canonical_bytes().unwrap(),
        explicit.canonical_bytes().unwrap()
    );

    let nested = "pub data Unit {}\npub trait Worker { machine register<machine Work>() where machine Work(); ; }\n";
    let absent = project(&Fixture::local(nested));
    let explicit = project(&Fixture::local(
        &nested.replace("machine Work()", "machine Work() -> Unit"),
    ));
    assert_eq!(absent.public_data(), explicit.public_data());
    assert_ne!(absent.public_traits(), explicit.public_traits());
    assert_ne!(
        absent.canonical_bytes().unwrap(),
        explicit.canonical_bytes().unwrap()
    );
}
