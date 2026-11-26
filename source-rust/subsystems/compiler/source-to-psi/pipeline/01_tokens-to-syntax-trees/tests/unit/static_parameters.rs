#[test]
fn retired_receiver_parameters_reject_in_every_signature_owner() {
    for parameter in [
        "self",
        "mutable self",
        "@self",
        "@mutable self",
        "@write self",
    ] {
        for source in [
            format!("data Counter {{ value: u32; }} machine Counter::step({parameter}) {{}}"),
            format!("trait Operation {{ machine step({parameter}); }}"),
            format!(
                "boundary trait Operation {{ machine step({parameter}) reaches none crashes none; }}"
            ),
            format!("machine run() {{ machine step({parameter}) {{}} }}"),
        ] {
            let error = crate::tests::parse_error(&source);
            crate::tests::expect_error(&error, "`self` receivers are retired");
        }
    }
}

#[test]
fn implicit_self_expressions_and_transition_targets_reject() {
    for body in [
        "self",
        "self.value",
        "transition { _ -> self }",
        "transition { _ -> self() }",
    ] {
        let source = format!("machine run() {{ {body} }}");
        let error = crate::tests::parse_error(&source);
        crate::tests::expect_error(&error, "implicit `self` is retired");
    }
}

#[test]
fn explicit_owned_shared_mutable_and_write_only_parameters_parse() {
    let source = r#"
        data Counter { value: u32; }
        trait Readable<Readout> { machine read(counter: @Readout) -> u32; }
        machine consume(counter: Counter) {}
        machine read(counter: @Counter) -> u32 { counter.value }
        machine write(counter: @write Counter) { counter.value = 1; }
        machine Counter::clear(counter: @mutable Counter) {
            transition { _ -> finish(counter) }
            machine finish(counter: @mutable Counter) { counter.value = 0; }
        }
    "#;
    crate::tests::parse_source(source);
}

#[test]
fn only_the_current_mutability_modifier_is_accepted() {
    for source in [
        "machine f(mut value: u8) {}",
        "machine f() { let mut local: u8 = 1; }",
        "machine f(cell: @mut u8) {}",
        "machine f<'cell>(cell: @'cell mut u8) {}",
        "machine f(value: u8) { let loan: @u8 = @mut value; }",
        "machine f(value: @mutable u8) { let loan: @u8 = value as @mut u8; }",
    ] {
        let error = crate::tests::parse_error(source);
        let span = error.source_span.span;
        assert!(span.end > span.start, "{source}");
    }
}

#[test]
fn an_ordinary_name_can_be_borrowed_after_the_at_sign() {
    let source = "data mut { value: u8; }
        machine read(value: @mut) -> u8 { value.value }
        machine f(mut: mut) -> u8 { read(@mut) }
        machine g() -> u8 { let mut: mut = mut { value: 3u8 }; read(@mut) }";
    crate::tests::parse_source(source);
}

#[test]
fn reference_introduction_uses_at_and_leaves_ampersand_as_an_operator() {
    for source in [
        "machine f(value: &u8) {}",
        "machine f(value: &mutable u8) {}",
        "machine f(value: &mut u8) {}",
        "machine f(value: &write u8) {}",
        "machine f(value: u8) { let loan: @u8 = &value; }",
        "machine f(value: u8) { let loan: @u8 = @state value; }",
    ] {
        crate::tests::parse_error(source);
    }
    let source = "machine f(mutable state: u8, mask: u8) -> u8 {
        let read: @u8 = @state;
        let mutable owned: u8 = state & mask;
        owned
    }";
    crate::tests::parse_source(source);
}
