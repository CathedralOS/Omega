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
    let good = "machine forward(view:@mut Counter)->@mut Counter { view }";
    check(schema, good).unwrap();
    refused(
        schema,
        &good.replace("view:@mut Counter", "view:@Counter"),
        "view",
        r"expected @mut game\Counter, found @game\Counter",
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
    let exclusive = shared.replace("item:@I", "item:@mut I");
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
