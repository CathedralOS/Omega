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

#[test]
fn generated_result_domain_selection_follows_each_call_destination() {
    let definitions = (
        "domains.omg",
        "domain u8::A; domain u8::B; data Pair{a:u8 in A;b:u8 in B;} machine take(v:u8 in A){}",
    );
    let overloads = [
        ("plain.omg", "machine pick(x:u8)->u8{x}"),
        ("a.omg", "machine pick(x:u8)->u8 in A{x as +A}"),
        ("b.omg", "machine pick(x:u8)->u8 in B{x as +B}"),
    ];
    // Each expected file names the declaration for one pick(x), in source order.
    let callers: &[(&str, &str, &[&str])] = &[
        ("return.omg", "machine direct(x:u8)->u8 in A{(pick(x))}", &["a.omg"]),
        ("local.omg", "machine local(x:u8)->u8 in A{let v:u8 in A=pick(x);v}", &["a.omg"]),
        ("assign.omg", "machine assign(x:u8)->u8 in B{let mut v:u8 in B=x as +B;v=pick(x);v}", &["b.omg"]),
        ("argument.omg", "machine argument(x:u8){take(pick(x))}", &["a.omg"]),
        ("fields.omg", "machine fields(x:u8)->Pair{Pair{b:pick(x),a:pick(x)}}", &["b.omg", "a.omg"]),
        (
            "match.omg",
            "machine arms(x:u8,b:bool)->u8 in B{match b{true->pick(x),false->pick(x),}}",
            &["b.omg", "b.omg"],
        ),
        (
            "transition.omg",
            "machine route(x:u8,b:bool)->u8 in A{transition b{true->pick(x),false->pick(x),}}",
            &["a.omg", "a.omg"],
        ),
        ("discard.omg", "machine discard(x:u8)->u8 in B{_ = pick(x);pick(x)}", &["plain.omg", "b.omg"]),
    ];
    let mut files = vec![definitions];
    files.extend(overloads);
    files.extend(callers.iter().map(|&(path, source, _)| (path, source)));
    let stage = check(&files).unwrap_or_else(|error| panic!("{error:?}"));
    for &(path, source, expected) in callers {
        let picks = stage
            .calls
            .iter()
            .filter(|call| call.site.file == path && call.machine.path == "pick");
        let selected: Vec<_> = picks
            .map(|call| {
                let signature = stage
                    .signatures
                    .iter()
                    .find(|sig| sig.machine == call.machine)
                    .unwrap();
                signature.site.file.as_str()
            })
            .collect();
        assert_eq!(selected, expected, "{source}");
        // Remove each required overload in turn. A refusal must point to the first
        // call that lost its producer, including when an earlier sibling still works.
        for (index, &missing) in expected.iter().enumerate() {
            if expected[..index].contains(&missing) {
                continue;
            }
            let mut twin = vec![definitions, (path, source)];
            twin.extend(overloads.into_iter().filter(|(file, _)| *file != missing));
            let error = check(&twin)
                .err()
                .unwrap_or_else(|| panic!("accepted {source} without {missing}"));
            let offset = source.match_indices("pick(x)").nth(index).unwrap().0;
            assert_eq!(error.file, path, "{source}");
            assert_eq!(error.message, "no matching overload for pick(u8)", "{source}");
            assert_eq!(error.span.get_start_offset() as usize, offset, "{source}");
            assert_eq!(error.span.slice(source.as_bytes()).unwrap(), b"pick", "{source}");
        }
    }
}

#[test]
fn domain_carriers_survive_without_a_typed_self_expression() {
    let facts = check(&[("domains.omg", "domain i32::Tag; public domain u16::Empty requires false;")]).unwrap();
    assert_eq!(facts.domain_carriers.len(), 2);
    for (path, signed, width) in [("i32::Tag", true, 32), ("u16::Empty", false, 16)] {
        let (declaration, carrier) = facts
            .domain_carriers
            .iter()
            .find(|(declaration, _)| declaration.package == "game" && declaration.path == path)
            .expect(path);
        assert_eq!(declaration.kind, omega_parser::NodeKind::Domain);
        spec_corpus::observe_handoff!("normalized-domain-carriers/producer", assert_eq!(*carrier, ValueType::Integer { signed, width: Some(width) }));
    }
}

#[test]
fn duplicate_overloads_compare_normalized_operands_and_result_domains_together() {
    let declarations = "data Byte=u8; domain<const N:u8> u8::Bound<N>; domain u8::Five=u8::Bound<5>;
        machine pick(tag:Byte,x:u8 in Five)->u8 in Five{x}";
    let duplicate = format!("{declarations} machine pick(tag:u8,x:u8 in Bound<2+3>)->u8 in u8::Bound<3+2>{{x}}");
    let error = check(&[("duplicate.omg", &duplicate)]).unwrap_err();
    assert_eq!(error.message, "duplicate typed operand signature for pick");
}

#[test]
fn result_domain_indices_follow_the_operand_binder_correspondence() {
    let declarations = "data Buffer<const N:u8>{} domain<const N:u8> u8::Label<N>;
        machine pick<const N:u8,const M:u8>(a:Buffer<N>, b:Buffer<M>, x:u8 in Label<N> & Label<M>)->u8 in Label<N>{x as =Label<N> -Label<M>}";
    for (index, removed, duplicate) in [("Y", "X", true), ("X", "Y", false)] {
        let candidate = format!(
            "{declarations} machine pick<const X:u8,const Y:u8>(a:Buffer<Y+0>, b:Buffer<X>, x:u8 in Label<X> & Label<Y>)->u8 in u8::Label<{index}+0>{{x as =Label<{index}> -Label<{removed}>}}"
        );
        let typed = check(&[("indices.omg", &candidate)]);
        if duplicate {
            assert_eq!(typed.unwrap_err().message, "duplicate typed operand signature for pick");
        } else {
            spec_corpus::observe_handoff!(
                "generic-parameter-correspondence/producer",
                assert_eq!(
                    typed
                        .unwrap()
                        .signatures
                        .iter()
                        .filter(|signature| signature.machine.path == "pick")
                        .count(),
                    2
                )
            );
        }
    }
}

#[test]
fn repeated_domain_conjuncts_compare_as_a_set_under_the_same_binder_correspondence() {
    let source = "data Buffer<const N:u8>{} domain<const N:u8> u8::Label<N>;
        machine pick<const N:u8,const M:u8>(a:Buffer<N>, b:Buffer<M>, x:u8 in Label<N> & Label<M>)->u8 in Label<N> & Label<M>{x}
        machine pick<const X:u8,const Y:u8>(a:Buffer<Y+0>, b:Buffer<X>, x:u8 in Label<X> & Label<Y>)->u8 in u8::Label<X+0> & u8::Label<Y+0>{x}";
    let error = check(&[("conjuncts.omg", source)]).unwrap_err();
    assert_eq!(error.message, "duplicate typed operand signature for pick");
}

#[test]
fn result_domain_runtime_subjects_compare_by_parameter_position() {
    let prefix = "data Byte=u8; domain<N:u8> u8::Label<N>;
        machine pick(a:Byte,b:u8,x:u8 in Label<a> & Label<b>)->u8 in Label<a>{x as =Label<a> -Label<b>}";
    for (index, removed, duplicate) in [("m", "n", true), ("n", "m", false)] {
        let source = format!("{prefix} machine pick(m:u8,n:u8,x:u8 in Label<n> & Label<m>)->u8 in Label<{index}>{{x as =Label<{index}> -Label<{removed}>}}");
        let typed = check(&[("subjects.omg", &source)]);
        if duplicate {
            assert_eq!(typed.unwrap_err().message, "duplicate typed operand signature for pick");
        } else {
            assert_eq!(
                typed
                    .unwrap()
                    .signatures
                    .iter()
                    .filter(|signature| signature.machine.path == "pick")
                    .count(),
                2
            );
        }
    }
}

#[test]
fn result_domain_overloads_follow_runtime_operand_order_across_files() {
    let definitions = ("domains.omg", "data Byte=u8; domain<N:u8> u8::Label<N>;");
    let overloads = [
        (
            "first.omg",
            "machine pick(a:Byte,b:u8,x:u8 in Label<a> & Label<b>)->u8 in Label<a>{x as =Label<a> -Label<b>}",
        ),
        (
            "second.omg",
            "machine pick(m:u8,n:u8,x:u8 in Label<n> & Label<m>)->u8 in Label<n>{x as =Label<n> -Label<m>}",
        ),
    ];
    let callers = [
        (
            "ordered.omg",
            "machine ordered(k:u8,m:u8,x:u8 in Label<k> & Label<m>)->u8 in Label<k>{pick(k,m,x)}",
            "first.omg",
        ),
        (
            "swapped.omg",
            "machine swapped(k:u8,m:u8,x:u8 in Label<k> & Label<m>)->u8 in Label<k>{pick(m,k,x)}",
            "second.omg",
        ),
    ];
    let mut files = vec![definitions];
    files.extend(overloads);
    files.extend(callers.iter().map(|&(file, source, _)| (file, source)));
    let stage = check(&files).unwrap();
    assert_eq!(stage.calls.len(), callers.len());
    for (file, source, expected) in callers {
        let call = stage.calls.iter().find(|call| call.site.file == file).unwrap();
        let selected = stage
            .signatures
            .iter()
            .find(|signature| signature.machine == call.machine)
            .unwrap();
        let caller = stage
            .signatures
            .iter()
            .find(|signature| signature.site.file == file)
            .unwrap();
        assert_eq!(selected.site.file, expected, "{source}");
        assert_eq!(call.result, caller.result, "{source}");

        let remaining = overloads
            .into_iter()
            .find(|&(path, _)| path != expected)
            .unwrap();
        let error = check(&[definitions, remaining, (file, source)]).unwrap_err();
        assert_eq!(error.file, file);
        assert!(error.message.starts_with("no matching overload for pick("), "{error:?}");
        assert_eq!(error.span.get_start_offset(), source.find("pick").unwrap());
        assert_eq!(error.span.slice(source.as_bytes()).unwrap(), b"pick");
    }
}
