// Whether a returned reference exposes invocation-local storage depends on the
// storage it denotes, never on how the return is spelled (lifetimes.md: "What
// qualifies is the backing a view denotes, not the form of the returning
// expression"). Each local spelling must be refused at the returned expression;
// each twin that returns caller storage must be accepted and hand that
// expression to the lifetime checker.

/// The byte range of the last occurrence of `text` in `source`.
fn last_occurrence(
    source: &str,
    text: &str,
) -> (usize, usize) {
    let start = source.rfind(text).unwrap_or_else(|| panic!("{text} is not in {source}"));
    (start, start + text.len())
}

#[test]
fn returned_views_of_local_storage_are_refused_however_spelled() {
    for (source, returned) in [
        ("machine f(x:u8)->@u8 { let v = @x; v }", "v"),
        ("machine f(x:u8)->@u8 { match @x { r -> r, } }", "match @x { r -> r, }"),
        ("machine f(x:u8)->@u8 { transition @x { r -> r, } }", "r"),
        ("machine f()->@u8 { let y:u8 = 1; match @y { r -> r, } }", "match @y { r -> r, }"),
        ("machine f(x:u8)->@u8 { let (v, _) = (@x, 0u8); v }", "v"),
        ("machine f(x:u8)->@u8 { let v = @x; move v }", "move v"),
        ("machine f(x:u8)->@u8 { @x as @u8 }", "@x as @u8"),
        ("machine f(x:u8)->@u8 { let v = (@x); let w = move v; (w as @u8) }", "(w as @u8)"),
        ("data V { r:@u8; } machine f(x:u8)->@u8 { let h = V { r: @x }; h.r }", "h.r"),
        ("data V { r:@u8; } machine f(x:u8)->@u8 { let V { r } = V { r: @x }; r }", "r"),
        ("data V { r:@u8; } data W { v:V; } machine f(x:u8)->@u8 { let w = W { v: V { r: @x } }; w.v.r }", "w.v.r"),
        ("machine f(x:u8)->@u8 { let a = [@x, @x]; a[0] }", "a[0]"),
        ("machine f(a:[u8;2])->@u8 { @a[0] }", "@a[0]"),
        (
            "data E { case A(r:@u8); case B; } machine f(x:u8, p:@u8)->@u8 { let e = E::A { r: @x }; match e { E::A { r } -> r, E::B -> p, } }",
            "match e { E::A { r } -> r, E::B -> p, }",
        ),
        ("data D { v:u8; } machine f()->@u8 { let y = D { v: 1u8 }; match @y { D { v } -> @v, } }", "match @y { D { v } -> @v, }"),
        ("machine f(x:u8, p:@u8)->@u8 { let mut v = p; v = @x; v }", "v"),
        ("machine f(x:u8, y:@u8, b:bool)->@u8 { match b { true -> @x, false -> y, } }", "match b { true -> @x, false -> y, }"),
        ("machine f(x:@u8)->@@u8 { @x }", "@x"),
    ] {
        let error = match check(&[("views.omg", source)]) {
            Err(error) => error,
            Ok(stage) => panic!("{source}: accepted with {:?}", stage.obligations.iter().map(|o| &o.kind).collect::<Vec<_>>()),
        };
        assert_eq!(error.message, "returned reference borrows invocation-local storage", "{source}");
        let (start, end) = last_occurrence(source, returned);
        assert_eq!((error.span.get_start_offset(), error.span.get_end_offset()), (start, end), "{source}");
    }
}

#[test]
fn returned_views_of_caller_storage_keep_their_return_obligation() {
    for (source, returned) in [
        ("machine f(x:@u8)->@u8 { match x { r -> r, } }", "match x { r -> r, }"),
        ("machine f(x:@u8)->@u8 { transition x { r -> r, } }", "r"),
        ("data D { v:u8; } machine f(d:@D)->@u8 { @d.v }", "@d.v"),
        ("data D { v:u8; } machine f(d:@D)->@u8 { match d { D { v } -> @v, } }", "match d { D { v } -> @v, }"),
        ("data D { v:u8; } machine f(d:@D)->@u8 { let D { v } = d; @v }", "@v"),
        ("data D { v:u8; w:u8; } machine f(d:@D)->@u8 { let (v, _) = d; @v }", "@v"),
        ("data I { n:u8; } data D { i:I; } machine f(d:@D)->@I { @d.i }", "@d.i"),
        ("data I { n:u8; } data D { i:I; } machine f(d:@D)->@I { match d { D { i } -> @i, } }", "match d { D { i } -> @i, }"),
        ("machine f(x:@u8, y:u8)->@u8 { let (v, _) = (x, @y); v }", "v"),
        ("machine f(x:@u8)->@u8 { let v = x; move v }", "move v"),
        ("machine f(x:@u8)->@u8 { x as @u8 }", "x as @u8"),
        ("data V { r:@u8; } machine f(x:@u8)->@u8 { let h = V { r: x }; h.r }", "h.r"),
        ("data V { r:@u8; } data W { v:V; } machine f(x:@u8, y:u8)->@u8 { let w = W { v: V { r: x } }; let _k = @y; w.v.r }", "w.v.r"),
        ("machine f(a:@[u8;2])->@u8 { @a[0] }", "@a[0]"),
        (
            "data E { case A(r:@u8); case B; } machine f(e:E, p:@u8)->@u8 { match e { E::A { r } -> r, E::B -> p, } }",
            "match e { E::A { r } -> r, E::B -> p, }",
        ),
        ("machine f(x:u8, p:@u8)->@u8 { let mut v = @x; v = p; v }", "v"),
    ] {
        let stage = check(&[("views.omg", source)]).unwrap_or_else(|error| panic!("{source}: {error:?}"));
        let (start, end) = last_occurrence(source, returned);
        assert!(
            stage.obligations.iter().any(|o| matches!(&o.kind, omega_typer::ObligationKind::ReturnBorrow { machine } if machine.path == "f")
                && (o.site.span.get_start_offset(), o.site.span.get_end_offset()) == (start, end)),
            "{source}: {:?}",
            stage.obligations
        );
    }
}
