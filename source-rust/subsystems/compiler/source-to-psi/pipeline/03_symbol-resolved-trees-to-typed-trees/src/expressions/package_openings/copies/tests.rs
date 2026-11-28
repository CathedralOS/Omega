//! Observable relationships between the fields of copied records.
#[test]
fn enclosing_copies_preserve_only_the_same_still_related_members() {
    let source = r#"
        data Read<S> = machine(input: @S) -> u8 reaches none crashes none;
        data Pair<some S> [copy] { left: Read<S>; right: Read<S>; }
        data Envelope [copy] { first: Pair; second: Pair; }
        machine repack(p: Envelope, q: Envelope, choose: bool) -> Pair {
            let mutable copied: Envelope = p;
            copied.second = q.second;
            let nested: Pair = copied.first;
            Pair { left: p.first.left, right: nested.right }
        }
    "#;
    let check = |text: &str| crate::qualified_operator_home_tests::checked(&[("main.omg", text)]);
    let p = check(source).expect("an independent sibling write leaves the copy intact");
    let [a, b] = p.package_projections.as_slice() else {
        panic!("two member reads")
    };
    assert_ne!(a.opening, b.opening);
    assert_eq!(
        p.package_opening(a.opening).witnesses,
        p.package_opening(b.opening).witnesses
    );
    for crossed in [
        source.replace("copied.first;", "copied.second;"),
        source.replace("= p;", "= q;"),
        source.replace("copied.second =", "copied.first ="),
    ] {
        assert!(
            check(&crossed)
                .unwrap_err()
                .contains("conflicting generic arguments")
        );
    }
    let branches = source.replace(
        "copied.second = q.second;",
        "let n: u8 = match choose { true -> { copied = p; 1u8 }, false -> { copied = p; 2u8 } };",
    );
    check(&branches).expect("both arms copy from the same input");
    let crossed = branches.replace("false -> { copied = p;", "false -> { copied = q;");
    assert!(
        check(&crossed)
            .unwrap_err()
            .contains("conflicting generic arguments")
    );
}
