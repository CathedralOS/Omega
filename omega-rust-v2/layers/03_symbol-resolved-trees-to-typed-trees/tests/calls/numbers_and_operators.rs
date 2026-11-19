#[test]
fn integers_land_at_the_destination_and_suffixes_keep_their_carrier() {
    let stage = check(&[("a.omg", "machine f() -> u8 { 255 }")]).unwrap();
    assert_eq!(stage.landings.len(), 1);
    for code in [
        "machine f() -> u8 { 256 }",
        "machine f() -> i1 { 1 }",
        "machine f() -> u32 { 1u8 }",
        "machine f() -> u8 { 256u8 }",
        "machine f(x: u0) {}",
    ] {
        assert!(check(&[("a.omg", code)]).is_err(), "{code}");
    }
    check(&[("a.omg", "machine f() -> u8 { 0xffu8 }")]).unwrap();
}

#[test]
fn arbitrary_precision_and_fractional_intermediates() {
    let stage = check(&[(
        "a.omg",
        "machine f() -> u257 {340282366920938463463374607431768211456} machine g()->u8{((5/2)*2) as u8} machine d()->f32{1.25e-1} machine min()->i8 {-128}",
    )])
    .unwrap();
    assert_eq!(stage.landings.len(), 4);
    for (expr, reason) in [
        ("(5/2)*2", "fractional intermediate"),
        ("(5/2) as u8", "found 5/2"),
        ("1/0", "division by zero"),
        ("1%2", "already typed"),
        ("-1", "expected u8"),
        ("256+1", "found 257"),
    ] {
        let code = format!("machine f()->u8{{ {expr} }}");
        assert!(check(&[("a.omg", &code)]).unwrap_err().message.contains(reason), "{expr}");
    }
}

#[test]
fn integer_landing_matches_independent_endpoint_oracle() {
    for width in [1, 2, 7, 8, 13, 24, 31, 63, 64, 104] {
        for signed in [false, true] {
            let limit = 1i128 << (width - i32::from(signed));
            let low = if signed { -limit } else { 0 };
            let high = limit - 1;
            for value in [low - 1, low, 0, high, high + 1] {
                let carrier = if signed { "i" } else { "u" };
                let source = format!("machine test()->{carrier}{width}{{ {value} }}");
                assert_eq!(check(&[("oracle.omg", &source)]).is_ok(), (low..=high).contains(&value), "{source}");
            }
        }
    }
}

#[test]
fn long_binary_chain_types_in_source_order_without_recursive_descent() {
    let source = format!("machine f(x:u8)->u8 {{ x{} }}", " +% 1".repeat(4096));
    let stage = check(&[("deep.omg", &source)]).unwrap();
    assert_eq!(stage.operations.len(), 4096);
    assert!(
        stage
            .operations
            .windows(2)
            .all(|pair| pair[0].site.span.get_end_offset() < pair[1].site.span.get_end_offset())
    );
}

#[test]
fn operator_grouping_survives_typing_with_each_intermediate_result() {
    for carrier in ["u8", "u32", "u64"] {
        let expression = "a +% b *% c == wanted && ready";
        let source = format!("machine f(a:{carrier},b:{carrier},c:{carrier},wanted:{carrier},ready:bool)->bool{{{expression}}}");
        let stage = check(&[("grouping.omg", &source)]).unwrap();
        let expected = [
            ("*%", "b *% c", carrier),
            ("+%", "a +% b *% c", carrier),
            ("==", "a +% b *% c == wanted", "bool"),
            ("&&", expression, "bool"),
        ];
        assert_eq!(stage.operations.len(), expected.len());
        for (token, text, result) in expected {
            let operation = stage.operations.iter().find(|op| op.token == token).unwrap();
            assert_eq!(operation.site.span.slice(source.as_bytes()).unwrap(), text.as_bytes());
            assert_eq!(operation.result.to_string(), result);
        }
        let wrong = source.replace("&& ready", "&& c");
        let error = check(&[("grouping.omg", &wrong)]).unwrap_err();
        assert_eq!(error.message, format!("no matching operator && for bool and {carrier}"));
    }
}
