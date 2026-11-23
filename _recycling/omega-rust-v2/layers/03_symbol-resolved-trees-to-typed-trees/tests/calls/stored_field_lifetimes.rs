#[test]
fn stored_references_require_a_lifetime_at_the_field_use() {
    for (declaration, annotation) in [
        ("data S { field: @u8; }", "@u8"),
        ("data Label { name: @[u8]; } machine leak<'n>(l:Label) -> @'n [u8] { l.name }", "@[u8]"),
        ("data Bytes<'t> { view: @'t [u8]; } data S<'t> { field: @Bytes<'t>; }", "@Bytes<'t>"),
        ("data S { case A(field: @write u8); case B; }", "@write u8"),
        ("data V = @mutable u8; data S { field: V; }", "V"),
        ("data S { field: [@u8; 2]; }", "[@u8; 2]"),
        ("data S<'a> { field: @'a [@u8]; }", "@'a [@u8]"),
        ("data Box<T> { field: T; } data S { field: Box<@u8>; }", "Box<@u8>"),
    ] {
        let error = check(&[("stored.omg", declaration)]).unwrap_err();
        assert_eq!(error.message, "a stored reference must name its lifetime", "{declaration}");
        assert_eq!(error.span.slice(declaration.as_bytes()), Some(annotation.as_bytes()), "{declaration}");
    }
}

#[test]
fn named_storage_and_elided_callable_signatures_are_accepted() {
    let source = "data View<'a> = @'a u8;
        data Callback = machine(x: @u8) -> @u8 reaches none crashes none;
        data Holder<'a> { views: [View<'a>; 2]; read: Callback; }
        machine identity(x: @u8) -> @u8 { x }";
    check(&[("stored.omg", source)]).unwrap();
}

#[test]
fn a_returned_stored_view_remains_tied_to_its_declared_lifetime() {
    let label = "data Label<'a> { name: @'a [u8]; }";
    let valid = format!("{label} machine get<'a>(l: Label<'a>) -> @'a [u8] {{ l.name }}");
    check(&[("label.omg", &valid)]).unwrap();
    let invalid = format!("{label} machine leak<'a, 'n>(l: Label<'a>) -> @'n [u8] {{ l.name }}");
    let error = check(&[("label.omg", &invalid)]).unwrap_err();
    assert_eq!(error.span.slice(invalid.as_bytes()), Some(b"l.name".as_slice()));
}
