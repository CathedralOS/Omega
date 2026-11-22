// Reading a field out of borrowed storage copies it, so the field needs copy
// eligibility and the read records `CopyRead` where it happens (patterns.md:
// "A field-value binding from borrowed storage still needs ordinary copy
// eligibility. Non-copy fields require an otherwise-authorized explicit
// borrow"). A field selection and a pattern binding of a borrowed subject read
// the same way as `let v = @d.n; $v`. `I` is affine: data is affine unless it
// says `[copy]`.

const BORROWED_FIELDS: &str = "data I { n:u8; } data D { i:I; n:u8; }";

/// Byte range of the last occurrence of `text` in `source`.
fn last_span(
    source: &str,
    text: &str,
) -> (usize, usize) {
    let start = source
        .rfind(text)
        .unwrap_or_else(|| panic!("{text} is not in {source}"));
    (start, start + text.len())
}

#[test]
fn non_copy_fields_cannot_be_read_out_of_shared_storage() {
    let refusal = "reading game\\I out of borrowed storage requires a copy type; borrow it with @ instead";
    for (body, read) in [
        ("machine f(d:@D)->I { d.i }", "d.i"),
        ("machine f(d:@D)->I { match d { D { i, n as _ } -> i, } }", "i"),
        ("machine f(d:@D)->I { let D { i, n as _ } = d; i }", "i"),
        ("machine f(d:@D)->I { let (i, _) = d; i }", "i"),
        ("machine f(d:@D)->I { let (i, ..) = d; i }", "i"),
        ("machine f(d:@D)->I { match d.i { j -> j, } }", "j"),
        ("machine f(d:@D)->I { move d.i }", "d.i"),
        ("data W { i:I; } machine f(d:@D)->W { let D { i, .. } = d; W { i } }", "i"),
    ] {
        let source = format!("{BORROWED_FIELDS} {body}");
        let error = match check(&[("fields.omg", &source)]) {
            Err(error) => error,
            Ok(stage) => panic!("{body}: accepted with {:?}", stage.obligations.iter().map(|o| &o.kind).collect::<Vec<_>>()),
        };
        assert_eq!(error.message, refusal, "{body}");
        assert_eq!((error.span.get_start_offset(), error.span.get_end_offset()), last_span(&source, read), "{body}");
    }
}

#[test]
fn copy_reads_of_borrowed_fields_record_the_read_where_it_happens() {
    for (body, read, ty) in [
        ("machine f(d:@D)->u8 { d.n }", "d.n", "u8"),
        ("machine f(d:@D)->u8 { d.i.n }", "d.i.n", "u8"),
        ("machine f(d:@D)->u8 { (d.i).n }", "(d.i).n", "u8"),
        ("machine f(d:@D)->u8 { let v = @d.n; $v }", "$v", "u8"),
        ("machine f(d:@D)->u8 { match d { D { n, .. } -> n, } }", "n", "u8"),
        ("machine f(d:@D)->u8 { let D { n, .. } = d; n }", "n", "u8"),
        (
            "data Reversed { n:u8; i:I; } machine f(d:@Reversed)->u8 { match d { (n, ..) -> n, } }",
            "n",
            "u8",
        ),
        ("machine f(d:@mutable D)->u8 { match d { D { n, .. } -> n, } }", "n", "u8"),
        ("machine f(d:@D)->u8 { match d.n { 0 -> 1u8, _ -> 2u8, } }", "d.n", "u8"),
        ("data W { n:u8; } machine f(d:@D)->W { let D { n, .. } = d; W { n } }", "n", "u8"),
    ] {
        let source = format!("{BORROWED_FIELDS} {body}");
        let stage = check(&[("fields.omg", &source)]).unwrap_or_else(|error| panic!("{body}: {error:?}"));
        let site = last_span(&source, read);
        let reads: Vec<_> = stage
            .obligations
            .iter()
            .filter_map(|o| match &o.kind {
                omega_typer::ObligationKind::CopyRead(read) => Some(((o.site.span.get_start_offset(), o.site.span.get_end_offset()), read.to_string())),
                _ => None,
            })
            .collect();
        assert_eq!(reads, [(site, ty.to_string())], "{body}");
    }
    let copy = "data I [copy] { n:u8; } data D { i:I; } machine f(d:@D)->I { d.i }";
    let stage = check(&[("fields.omg", copy)]).unwrap();
    assert!(
        stage
            .obligations
            .iter()
            .any(|o| matches!(&o.kind, omega_typer::ObligationKind::CopyRead(ty) if ty.to_string() == "game\\I")
                && (o.site.span.get_start_offset(), o.site.span.get_end_offset()) == last_span(copy, "d.i"))
    );
}

#[test]
fn owned_moves_borrows_and_mut_move_out_read_nothing() {
    for body in [
        "machine f(d:D)->u8 { d.i.n }",
        "machine f(d:D)->I { d.i }",
        "machine f(d:D)->I { match d { D { i, n as _ } -> i, } }",
        "machine f(d:D)->I { let D { i, n as _ } = d; i }",
        "machine f(d:@D)->@I { @d.i }",
        "machine f(d:@D)->@I { match d { D { i, .. } -> @i, } }",
        "machine f(d:@D)->@I { match d.i { j -> @j, } }",
        "machine f(d:@mutable D)->I { move d.i }",
        "machine f(d:@mutable D)->I { d.i }",
    ] {
        let source = format!("{BORROWED_FIELDS} {body}");
        let stage = check(&[("fields.omg", &source)]).unwrap_or_else(|error| panic!("{body}: {error:?}"));
        assert!(
            !stage
                .obligations
                .iter()
                .any(|o| matches!(o.kind, omega_typer::ObligationKind::CopyRead(_))),
            "{body}: {:?}",
            stage.obligations
        );
    }
    let write_only = format!("{BORROWED_FIELDS} machine f(d:@write D)->u8 {{ d.n }}");
    let error = check(&[("fields.omg", &write_only)]).unwrap_err();
    assert_eq!(error.message, "cannot read through a write-only reference");
    assert_eq!(error.span.slice(write_only.as_bytes()), Some(b"d.n".as_slice()));
}

#[test]
fn nested_mutable_references_keep_the_access_of_their_outer_loan() {
    let defs = "data Cell{v:u8;} data Handles<'a>{cell:@'a mutable Cell; bytes:@'a mutable [u8];}";
    for write in [
        "h.cell.v=1;",
        "_=@mutable (h.cell).v;",
        "h.bytes[0]=1;",
        "let Handles{cell,..} = h; cell.v=1;",
    ] {
        let example = |outer: &str| format!("{defs} machine run<'a>(h:{outer}Handles<'a>){{{write}}}");
        check(&[("loan.omg", &example("@mutable "))]).unwrap();
        check(&[("loan.omg", &example(""))]).unwrap();
        let shared = example("@");
        let error = check(&[("loan.omg", &shared)]).unwrap_err();
        assert!(error.message.contains("authority"), "{shared}: {error:?}");
    }
}

#[test]
fn a_declared_index_machine_reads_a_borrowed_field_only_for_a_value_parameter() {
    let defs = "data B{n:u8;} data Holder{item:B;} machine [] at(b:B,i:u64)->u8{b.n} machine read(h:@Holder)->u8{h.item[0]}";
    let error = check(&[("index.omg", defs)]).unwrap_err();
    assert_eq!(error.span.slice(defs.as_bytes()), Some(b"h.item".as_slice()));
    for (source, copied) in [
        (defs.replace("data B{", "data B [copy]{"), true),
        (defs.replace("at(b:B", "at(b:@B"), false),
    ] {
        let stage = check(&[("index.omg", &source)]).unwrap();
        assert_eq!(
            stage
                .obligations
                .iter()
                .any(|o| o.site.span.slice(source.as_bytes()) == Some(b"h.item".as_slice()) && matches!(&o.kind, omega_typer::ObligationKind::CopyRead(_))),
            copied
        );
    }
    let access = "data B{n:u8;} data H<'a>{r:@'a mutable B;} machine [] at(b:@mutable B,i:u64)->@mutable u8{@mutable b.n} machine f<'a>(h:@H<'a>){h.r[0]=1;}";
    assert!(check(&[("index.omg", access)]).is_err());
    check(&[("index.omg", &access.replace("(h:@H<'a>)", "(h:H<'a>)"))]).unwrap();
}

#[test]
fn named_and_indirect_parameters_decide_whether_to_read_a_borrowed_field() {
    let defs = "data B{n:u8;} data Holder{item:B;} data Inspector=machine(b:@B)->u8 reaches none crashes none; machine inspect(b:@B)->u8{b.n}";
    for (parameter, expr) in [("", "inspect((h.item))"), (",cb:Inspector", "cb((h.item))")] {
        let source = format!("{defs} machine run(h:@Holder{parameter})->u8{{{expr}}}");
        let stage = check(&[("read.omg", &source)]).unwrap();
        let occurrence = |site: &omega_typer::Site| site.span.slice(source.as_bytes()) == Some(b"(h.item)".as_slice());
        assert!(
            stage
                .obligations
                .iter()
                .any(|o| matches!(&o.kind,omega_typer::ObligationKind::Borrow { place,access:omega_typer::Access::Shared } if occurrence(place)))
        );
        assert!(
            !stage
                .obligations
                .iter()
                .any(|o| matches!(&o.kind,omega_typer::ObligationKind::CopyRead(t) if matches!(t,ValueType::Nominal(id) if id.path=="B")))
        );
        let by_value = source.replace("b:@B", "b:B");
        let error = check(&[("read.omg", &by_value)]).unwrap_err();
        assert_eq!(error.span.slice(by_value.as_bytes()), Some(b"(h.item)".as_slice()));
    }
}

#[test]
fn grouped_fields_passed_by_value_keep_the_transfer_on_the_projection() {
    use omega_typer::ObligationKind::{CopyRead, RestoreBorrowed};

    for (property, access, replace) in [(" [copy]", "@", ""), ("", "@mutable ", "h.item=B{n:0};")] {
        let source = format!(
            "data B{property} {{n:u8;}} data H{{item:B;}} data W{{b:B;}} machine wrap(b:B)->W{{W{{b:b}}}} machine run(h:{access}H)->W{{let w=wrap((h.item));{replace}w}}"
        );
        let resolved = support::resolve_packages(&[("game", "read.omg", &source)], &[]);
        let typed = omega_typer::type_stage(&resolved).unwrap();
        let tree = typed.trees().next().unwrap();
        let file = tree.resolved().file;
        let [call] = typed.calls.as_slice() else { panic!("expected one named call") };
        assert_eq!(call.machine.path, "wrap");
        let [operand] = call.operands.as_slice() else { panic!("expected one grouped operand") };
        assert_eq!(operand.site.span.slice(source.as_bytes()), Some(b"(h.item)".as_slice()));
        let field = file.node_id(file.ungroup(operand.node.node));
        let mut transfers = typed.obligations.iter().filter(|o| matches!(o.kind, CopyRead(_) | RestoreBorrowed { .. }));
        let transfer = transfers.next().expect("field passed by value");
        assert!(transfers.next().is_none());
        assert_eq!(transfer.node, field);
        assert_eq!(transfer.site.span.slice(source.as_bytes()), Some(b"h.item".as_slice()));
        match (&transfer.kind, access) {
            (CopyRead(ty), "@") => assert_eq!(ty.to_string(), "game\\B"),
            (RestoreBorrowed { place, ty }, "@mutable ") => {
                assert_eq!(*place, field);
                assert_eq!(ty.to_string(), "game\\B");
            }
            _ => panic!("{access}: {transfer:?}"),
        }
    }
}

#[test]
fn a_record_field_value_pattern_reads_the_field_of_a_named_lifetime_subject() {
    for reference in ["@", "@'a "] {
        let source = format!("data D{{flag:bool;}} machine choose<'a>(d:{reference}D)->bool{{match d {{D{{flag:true}}->true,_->false,}}}}");
        let stage = check(&[("pattern.omg", &source)]).unwrap();
        assert!(
            stage
                .obligations
                .iter()
                .any(|o| matches!(o.kind, omega_typer::ObligationKind::CopyRead(ValueType::Bool))
                    && o.site.span.slice(source.as_bytes()) == Some(b"flag:true".as_slice())),
            "{reference}"
        );
    }
}

#[test]
fn selecting_a_held_sum_case_applies_only_to_that_exact_holder_field() {
    let src = "data Token{v:u8;} data Choice{case A(left:Token);case B(left:Token);} data Holder{first:Choice;second:Choice;} machine pick(h:Holder)->Token{match h.first{Choice::A{..}->move h.first.left,Choice::B{..}->move h.first.left,}}";
    let stage = check(&[("held.omg", src)]).unwrap();
    let fields: Vec<_> = stage
        .expressions
        .iter()
        .filter(|e| e.site.span.slice(src.as_bytes()) == Some(b"h.first.left".as_slice()))
        .filter_map(|e| {
            let (_, steps) = stage.place_path(e.node);
            match steps.last().and_then(|step| step.projection.as_ref()) {
                Some(omega_typer::Projection::Field(field)) => Some(field.path.as_str()),
                _ => None,
            }
        })
        .collect();
    spec_corpus::observe_handoff!("subject-and-field-identity-across-matches/producer", assert!(fields.contains(&"Choice::A::left") && fields.contains(&"Choice::B::left")));
    let wrong = src.replacen("move h.first.left", "move h.second.left", 1);
    let err = check(&[("held.omg", &wrong)]).unwrap_err();
    spec_corpus::observe_handoff!("subject-and-field-identity-across-matches/refusal", assert_eq!(err.message, "Choice has no field left"));
    assert_eq!(err.span.slice(wrong.as_bytes()), Some(b"h.second.left".as_slice()));
}
