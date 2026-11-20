// Every caller in a family shares its declarations. Compile the valid callers
// together, then run missing-result cases separately to inspect each diagnostic.
#[test]
fn generated_result_domain_overloads_select_the_exact_set() {
    let domains = "domain u8::Parent::Child; domain u8::Other; domain u8::Pair=u8::Parent & u8::Other; domain u8::Parent;";
    let producers = [
        ("u8", "x"),
        ("u8 in Parent", "x as +Parent"),
        ("u8 in Other", "x as +Other"),
        ("u8 in u8::Parent::Child", "x as +Parent +u8::Parent::Child"),
        ("u8 in Parent & Other", "x as +Parent +Other"),
        ("u8 in u8::Parent::Child & Other", "x as +Parent +u8::Parent::Child +Other"),
    ];
    // The producer number is the independently expected declaration identity.
    let destinations = [
        (0, "u8"),
        (1, "u8 in Parent"),
        (2, "u8 in Other"),
        (3, "u8 in u8::Parent::Child"),
        (3, "u8 in Parent & u8::Parent::Child"),
        (4, "u8 in Other & Parent"),
        (4, "u8 in Pair"),
        (5, "u8 in Other & u8::Parent::Child"),
        (5, "u8 in Pair & u8::Parent::Child"),
    ];
    let declarations: Vec<_> = producers
        .iter()
        .enumerate()
        .map(|(i, (result, body))| (format!("overload_{i}.omg"), format!("machine pick(x:u8)->{result}{{{body}}}")))
        .collect();
    let callers: Vec<_> = destinations
        .iter()
        .enumerate()
        .map(|(i, (_, result))| (format!("caller_{i}.omg"), format!("machine caller_{i}(x:u8)->{result}{{pick(x)}}")))
        .collect();
    let all = (1usize << producers.len()) - 1;
    for family in 1..=all {
        let mut files = vec![("domains.omg", domains)];
        for (i, (path, source)) in declarations.iter().enumerate() {
            if family & (1 << i) != 0 {
                files.push((path.as_str(), source.as_str()));
            }
        }
        let declarations_end = files.len();
        for ((producer, _), (path, source)) in destinations.iter().zip(&callers) {
            if family & (1 << producer) != 0 {
                files.push((path, source));
            }
        }
        let stage = check(&files).unwrap_or_else(|error| {
            let Some(&caller) = files[declarations_end..]
                .iter()
                .find(|(path, _)| *path == error.file)
            else {
                panic!("family {family:#08b}, declaration error: {error:?}");
            };
            let mut reduced = files[..declarations_end].to_vec();
            reduced.push(caller);
            // Keep the same call-site diagnostic and retain one declaration so
            // that removing all producers cannot turn a typing error into a lookup error.
            if check(&reduced).is_err_and(|e| e == error) {
                for i in (1..declarations_end).rev() {
                    if reduced.len() == 3 {
                        break;
                    }
                    let candidate = reduced.remove(i);
                    if !check(&reduced).is_err_and(|e| e == error) {
                        reduced.insert(i, candidate);
                    }
                }
            } else {
                reduced = files.clone();
            }
            let program = reduced
                .iter()
                .map(|(_, src)| *src)
                .collect::<Vec<_>>()
                .join("\n");
            panic!("{error:?}\n{program}");
        });
        assert_eq!(stage.calls.len(), files.len() - declarations_end);
        for ((producer, _), (path, source)) in destinations.iter().zip(&callers) {
            if family & (1 << producer) != 0 {
                let call = stage
                    .calls
                    .iter()
                    .find(|call| call.site.file == *path)
                    .expect(source);
                let selected = stage
                    .signatures
                    .iter()
                    .find(|sig| sig.machine == call.machine)
                    .expect("selected declaration");
                assert_eq!(selected.site.file, declarations[*producer].0, "{source} with family {family:#08b}");
            } else if family == (all ^ (1 << producer)) {
                // Five competing results remain. Neither a subset nor a superset may satisfy this caller.
                let mut single = files[..declarations_end].to_vec();
                single.push((path, source));
                let err = check(&single)
                    .err()
                    .unwrap_or_else(|| panic!("unexpected acceptance: {source} with family {family:#08b}"));
                assert_eq!(err.file, *path);
                assert_eq!(err.span.slice(source.as_bytes()).unwrap(), b"pick");
                assert_eq!(err.message, "no matching overload for pick(u8)", "{source}");
            }
        }
    }
}

#[test]
fn a_generic_result_supplies_the_nested_calls_expected_domain() {
    let declarations = "domain u8::WireByte;
        machine identity<T>(value:T)->T { value }
        machine keep_value<Label,T>(label:Label, value:T)->T { value }
        machine is_valid<T>(value:T)->bool { true }
        data Envelope<T> { payload:T; }
        machine pack<T>(value:T)->Envelope<T> { Envelope::<T>{payload:value} }";
    let plain = ("plain.omg", "machine encode(value:u8)->u8 { value }");
    let wire = ("wire.omg", "machine encode(value:u8)->u8 in WireByte { value as +WireByte }");
    let caller = (
        "packet.omg",
        "machine inferred(value:u8)->u8 in WireByte { identity(encode(value)) }
        machine explicit(value:u8)->u8 in WireByte { identity::<u8 in WireByte>(encode(value)) }
        machine typed_local(value:u8)->u8 in WireByte { let byte:u8 in WireByte = encode(value); identity(byte) }
        machine nested(value:u8)->u8 in WireByte { identity(identity(encode(value))) }
        machine wrapped(value:u8)->Envelope<u8 in WireByte> { pack(encode(value)) }
        machine partial(value:u8)->u8 in WireByte { keep_value(true, encode(value)) }
        machine plain_result(value:u8)->u8 { identity(encode(value)) }
        machine unrelated(value:u8)->bool { is_valid(encode(value)) }
        machine unconstrained(value:u8) { let byte = identity(encode(value)); }",
    );
    let stage = check(&[("types.omg", declarations), plain, wire, caller]).unwrap();
    let mut encodings = 0;
    for call in stage.calls.iter().filter(|call| call.machine.path == "encode") {
        let caller = &call.caller.as_ref().unwrap().path;
        let file = match caller.as_str() {
            "inferred" | "explicit" | "typed_local" | "nested" | "wrapped" | "partial" => wire.0,
            "plain_result" | "unrelated" | "unconstrained" => plain.0,
            _ => panic!("unexpected caller {caller}"),
        };
        let selected = stage
            .signatures
            .iter()
            .find(|sig| sig.machine == call.machine)
            .unwrap();
        assert_eq!(selected.site.file, file, "{caller}");
        encodings += 1;
    }
    assert_eq!(encodings, 9);

    let inferred = ("packet.omg", "machine packet(value:u8)->u8 in WireByte { identity(encode(value)) }");
    check(&[("types.omg", declarations), wire, inferred]).unwrap();
    let error = check(&[("types.omg", declarations), plain, inferred]).unwrap_err();
    assert_eq!(error.message, "no matching overload for encode(u8)");
    assert_eq!(error.span.slice(inferred.1.as_bytes()), Some(b"encode".as_slice()));
}

#[test]
fn differing_outer_parameter_types_do_not_choose_an_inner_overload() {
    let declarations = (
        "wire.omg",
        "domain u8::WireByte;
        machine encode(value:u8)->u8 in WireByte { value as +WireByte }
        machine choose<T>(value:T, fallback:T)->T { value }
        machine choose<T>(value:u8, fallback:T)->T { fallback }",
    );
    let caller = (
        "packet.omg",
        "machine packet(value:u8, fallback:u8 in WireByte)->u8 in WireByte { choose(encode(value), fallback) }",
    );
    let error = check(&[declarations, caller]).unwrap_err();
    assert_eq!(error.message, "no matching overload for encode(u8)");
    let plain = ("plain.omg", "machine encode(value:u8)->u8 { value }");
    let stage = check(&[declarations, caller, plain]).unwrap();
    let encode = stage
        .calls
        .iter()
        .find(|call| call.machine.path == "encode")
        .unwrap();
    assert!(encode.result.domains().is_empty());
}

#[test]
fn two_applications_of_one_result_domain_do_not_order_the_type_arguments() {
    let tag = "domain<T> u8::Label<T>;
        machine tag<A,B>(left:A, right:B)->u8 in Label<A> & Label<B> { 0u8 as +Label<A> +Label<B> }";
    for (left, right) in [("bool", "u16"), ("u16", "bool")] {
        let source = format!("{tag} machine run(left:{left},right:{right})->u8 in Label<bool> & Label<u16> {{ tag(left,right) }}");
        let stage = check(&[("tag.omg", &source)]).unwrap();
        let call = stage
            .calls
            .iter()
            .find(|call| call.machine.path == "tag")
            .unwrap();
        assert_eq!(
            call.type_arguments
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            [left, right]
        );
    }
    let ambiguous = "domain<T> u8::Label<T>;
        machine tag<A,B>()->u8 in Label<A> & Label<B> { 0u8 as +Label<A> +Label<B> }
        machine run()->u8 in Label<bool> & Label<u16> { tag() }";
    assert!(check(&[("tag.omg", ambiguous)]).is_err());
}
