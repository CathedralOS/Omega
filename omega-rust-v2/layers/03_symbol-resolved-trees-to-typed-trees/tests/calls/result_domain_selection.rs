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
