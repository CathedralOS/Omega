mod support;

use omega_typer::{ObligationKind, TypedStage};

const BODY: &str = "landing.omg";

fn check(
    schema: &str,
    body: &str,
) -> Result<TypedStage, omega_typer::TypeError> {
    support::check(&[("types.omg", schema), (BODY, body)])
}

// The returned operand (not its declaration or the destination annotation)
// owns the refusal. Every fixture ends in `{ operand }`.
fn refused(
    schema: &str,
    body: &str,
    operand: &str,
    message: &str,
) {
    let prefix = body.strip_suffix(&format!("{{ {operand} }}")).unwrap();
    let start = prefix.len() + "{ ".len();
    let end = start + operand.len();
    let error = check(schema, body).unwrap_err();
    assert!(!error.waiting_for_static_value());
    assert_eq!((&*error.package, &*error.file), ("game", BODY));
    assert_eq!(error.span, omega_lexer::SourceSpan::new(start, end));
    assert_eq!(error.span.slice(body.as_bytes()), Some(operand.as_bytes()));
    assert_eq!(error.message, message);
    #[cfg(feature = "diagnostic-origins")]
    {
        assert!(
            error
                .diagnostic_origin()
                .file()
                .replace('\\', "/")
                .ends_with("/expressions/value_landing.rs")
        );
        support::report_return(&error, "", Some(("game", BODY, start, end, message)));
    }
}

#[test]
fn the_right_carrier_needs_the_destination_qualification_and_index() {
    let schema = "domain u8::P; domain<const N:u8> u8::Bound<N>;";
    let good = "machine keep(x:u8 in P)->u8 in P { x }";
    check(schema, good).unwrap();
    refused(schema, &good.replace("x:u8 in P", "x:u8"), "x", r"expected u8 in game\u8::P, found u8");

    let good = "machine keep(x:u8 in Bound<6>)->u8 in Bound<6> { x }";
    check(schema, good).unwrap();
    refused(
        schema,
        &good.replace("x:u8 in Bound<6>", "x:u8 in Bound<5>"),
        "x",
        r"expected u8 in game\u8::Bound<6>, found u8 in game\u8::Bound<5>",
    );
}

#[test]
fn forwarding_a_shared_reference_cannot_grant_mutable_authority() {
    let schema = "data Counter { n:u8; }";
    let good = "machine forward(view:@mutable Counter)->@mutable Counter { view }";
    check(schema, good).unwrap();
    refused(
        schema,
        &good.replace("view:@mutable Counter", "view:@Counter"),
        "view",
        r"expected @mutable game\Counter, found @game\Counter",
    );
}

#[test]
fn forwarding_must_preserve_both_the_referent_and_the_borrowed_region() {
    let good = "machine pack<'a,'b>(left:@'a u8,right:@'b u8)->@'b u8 { right }";
    check("", good).unwrap();
    refused(
        "",
        &good.replace("{ right }", "{ left }"),
        "left",
        r"expected @u8 at game\pack<parameter 1>, found @u8 at game\pack<parameter 0>",
    );

    let schema = "data File {} data Socket {}";
    let good = "machine pack<'a>(view:@'a Socket)->@'a Socket { view }";
    check(schema, good).unwrap();
    refused(
        schema,
        &good.replace("view:@'a Socket", "view:@'a File"),
        "view",
        r"expected @game\Socket at game\pack<parameter 0>, found @game\File at game\pack<parameter 0>",
    );
}

#[test]
fn equal_record_layout_does_not_substitute_a_different_data_carrier() {
    let schema = "data File { id:u8; } data Socket { id:u8; }";
    let good = "machine keep(handle:Socket)->Socket { handle }";
    check(schema, good).unwrap();
    refused(
        schema,
        &good.replace("handle:Socket", "handle:File"),
        "handle",
        r"expected game\Socket, found game\File",
    );
}

#[test]
fn an_array_destination_requires_array_shape_and_extent() {
    let good = "machine flags()->[bool;2] { [true,false] }";
    check("", good).unwrap();
    refused("", &good.replace(";2", ";1"), "[true,false]", "expected [bool; 1], found [bool; 2]");
    refused(
        "",
        &good.replace("[true,false]", "(true,false)"),
        "(true,false)",
        "expected [bool; 2], found (bool, bool)",
    );
}

#[test]
fn affine_read_needs_exclusive_storage_while_a_copy_read_only_needs_shared_storage() {
    let schema = "data I { n:u8; }";
    let shared = "machine extract(item:@I)->I { $item }";
    refused(
        schema,
        shared,
        "$item",
        r"reading game\I out of borrowed storage requires a copy type; borrow it with @ instead",
    );

    // Same dereference, two different legal transfers. Only the exclusive
    // move owes restoration; adding copy eligibility must not create that debt.
    let exclusive = shared.replace("item:@I", "item:@mutable I");
    for (schema, body, moving) in [
        (schema, exclusive.as_str(), true),
        ("data I [copy] { n:u8; }", shared, false),
    ] {
        let stage = check(schema, body).unwrap();
        let transfers: Vec<_> = stage
            .obligations
            .iter()
            .filter_map(|o| match &o.kind {
                ObligationKind::CopyRead(ty) => Some((false, ty.to_string(), o.site.span)),
                ObligationKind::RestoreBorrowed { ty, .. } => Some((true, ty.to_string(), o.site.span)),
                _ => None,
            })
            .collect();
        let start = body.find("$item").unwrap();
        assert_eq!(
            transfers,
            [(moving, r"game\I".to_string(), omega_lexer::SourceSpan::new(start, start + "$item".len()))]
        );
    }
}

#[test]
fn a_reference_is_not_implicitly_read_and_an_explicit_read_needs_read_authority() {
    let good = "machine read(view:@u8)->u8 { $view }";
    check("", good).unwrap();
    refused("", &good.replace("$view", "view"), "view", "expected u8, found @u8");
    refused(
        "",
        &good.replace("view:@u8", "view:@write u8"),
        "$view",
        "cannot read through a write-only reference",
    );
}

/// The type L3 publishes for the expression spelled `text` in the body.
fn published(
    stage: &TypedStage,
    body: &str,
    text: &str,
) -> omega_typer::ValueType {
    let start = body.rfind(text).unwrap();
    let mut found = stage
        .expressions
        .iter()
        .filter(|e| e.site.file == BODY && (e.site.span.get_start_offset(), e.site.span.get_end_offset()) == (start, start + text.len()));
    let ty = found
        .next()
        .unwrap_or_else(|| panic!("no expression {text}"))
        .ty
        .clone();
    assert!(found.next().is_none(), "two expressions {text}");
    ty
}

/// An array literal builds an owned `[T]` field at that field's own type, for
/// any element count, as `Issuance::Only { routes: [] }` does in
/// resources/access.md. It is the destination's `[T]` with the destination's
/// element, not a `[T; N]` value converted afterwards. An existing `[T; N]`
/// value, a view destination, a fixed length and no destination keep their
/// refusals.
#[test]
fn an_array_literal_builds_an_owned_slice_field_at_its_element_type() {
    use omega_typer::ValueType::{Array, Integer, Parameter, Slice};
    let u8_type = Integer { signed: false, width: Some(8) };
    let schema = "data Items<T> { items: [T]; } data Bytes { items: [u8]; } data Pair { items: [u8; 2]; }";

    // A closed generic field, empty and with elements whose type comes only
    // from the field (unsuffixed numbers).
    for (operand, elements) in [
        ("Items::<u8>{items: []}", &[][..]),
        ("Items::<u8>{items: [1, 2]}", &["1", "2"][..]),
    ] {
        let body = format!("machine f() -> Items<u8> {{ {operand} }}");
        let stage = check(schema, &body).unwrap();
        let literal = &operand[operand.find('[').unwrap()..operand.len() - 1];
        assert_eq!(published(&stage, &body, literal), Slice(Box::new(u8_type.clone())), "{operand}");
        for element in elements {
            let start = body.rfind(literal).unwrap() + literal.find(element).unwrap();
            assert!(
                stage
                    .landings
                    .iter()
                    .any(|l| l.site.span.get_start_offset() == start && l.ty == u8_type),
                "{element}"
            );
        }
    }

    // An open generic field keeps the machine's own parameter as its element.
    let body = "machine f<T>() -> Items<T> { Items::<T>{items: []} }";
    let stage = check(schema, body).unwrap();
    let Slice(element) = published(&stage, body, "[]") else {
        panic!("expected an owned [T]")
    };
    assert!(
        matches!(element.as_ref(), Parameter { owner, index: 0 } if owner.path.as_str() == "f"),
        "{element:?}"
    );

    // A case payload with a suffixed element: the same type, no `[u8; 1]`.
    let body = "data Choice { case Only(items: [u8]); case None; } machine f() -> Choice { Choice::Only{items: [7u8]} }";
    let stage = check("", body).unwrap();
    assert_eq!(published(&stage, body, "[7u8]"), Slice(Box::new(u8_type.clone())));
    assert_eq!(published(&stage, body, "7u8"), u8_type);

    // The fixed-length twin still publishes the array itself.
    let body = "machine f() -> Pair { Pair{items: [1, 2]} }";
    let stage = check(schema, body).unwrap();
    assert_eq!(published(&stage, body, "[1, 2]"), Array(Box::new(u8_type.clone()), 2));

    // Refusals, each at its exact site. A wrong element is refused at the
    // element. A wrong length, an existing array value and a view keep the
    // baseline diagnostics, and a literal with no destination still needs one.
    for (body, site, message) in [
        ("machine f() -> Bytes { Bytes{items: [true]} }", "true", "expected u8, found bool"),
        ("machine f() -> Bytes { Bytes{items: [300]} }", "300", "expected u8, found 300"),
        ("machine f() -> Pair { Pair{items: [1u8]} }", "[1u8]", "expected [u8; 2], found [u8; 1]"),
        (
            "machine f() -> Bytes { let a: [u8; 1] = [1]; Bytes{items: a} }",
            "a",
            "expected [u8], found [u8; 1]",
        ),
        ("machine f() { let v: @[u8] = []; }", "[]", "array literal needs an element type"),
        ("machine f() { let xs = []; }", "[]", "array literal needs an element type"),
    ] {
        refused_at(schema, body, site, message);
    }
}

/// The typer refuses `body` at the last occurrence of `site`.
fn refused_at(
    schema: &str,
    body: &str,
    site: &str,
    message: &str,
) {
    let start = body.rfind(site).unwrap();
    let error = check(schema, body).unwrap_err();
    assert!(!error.waiting_for_static_value());
    assert_eq!((&*error.package, &*error.file, &*error.message), ("game", BODY, message), "{body}");
    assert_eq!(error.span, omega_lexer::SourceSpan::new(start, start + site.len()), "{body}");
}
