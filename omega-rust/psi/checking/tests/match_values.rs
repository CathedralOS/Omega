fn diagnostics(source: &str) -> Vec<String> {
    let typed = crate::front_end::typed_program(source);
    validation::validate_program(&typed)
        .err()
        .unwrap_or_default()
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
}

#[test]
fn computed_match_results_preserve_arithmetic_policy_until_explicit_erasure() {
    for first in ["value + 1", "(1 as u64 in Wrapping) + 1", "~value"] {
        for second in ["other + 1", "2 as u64 in Saturating", "2u64"] {
            let source = format!(
                "machine run(flag: bool, value: u64 in Wrapping, other: u64 in Saturating) -> u64 {{
                    (match flag {{ true -> {first}, false -> {second} }}) as u64
                }}"
            );
            let errors = diagnostics(&source);
            assert!(
                errors
                    .iter()
                    .any(|error| error.contains("match arms produce incompatible")),
                "{source}: {errors:?}"
            );
        }
    }
    for policy in ["Wrapping", "Saturating", "Trapping"] {
        let source = format!(
            "machine run(flag: bool, value: u64 in {policy}) -> u64 {{
                match flag {{ true -> (value + 1) as u64, false -> 2u64 }}
            }}"
        );
        assert!(diagnostics(&source).is_empty(), "{source}");
    }
}

#[test]
fn match_checks_every_arm_and_requires_actual_coverage() {
    for (source, expected) in [
        (
            "machine run(flag: bool) -> bool { match flag { true -> true } }",
            "does not cover",
        ),
        (
            "machine run(value: i64) -> i64 { match value { 0 -> 1, 1 -> 2 } }",
            "does not cover",
        ),
        (
            "machine run(flag: bool) -> bool { match flag { true -> true, _ -> 7 } }",
            "incompatible",
        ),
        (
            "machine run(flag: bool) -> bool { match flag { 7 -> true, _ -> false } }",
            "incompatible",
        ),
        (
            "machine run(flag: bool) -> bool { match flag { _ -> true, false -> 7 } }",
            "incompatible",
        ),
        (
            "data Value { number: i64; } machine run(subject: Value, pattern: Value) -> bool { match subject { pattern -> true, _ -> false } }",
            "require scalar",
        ),
    ] {
        let errors = diagnostics(source);
        assert!(
            errors.iter().any(|error| error.contains(expected)),
            "{source}: {errors:?}"
        );
    }
}

#[test]
fn skipped_match_calls_keep_type_and_scope_checks_but_not_preconditions() {
    let prefix = "machine need(flag: bool) -> bool requires flag == true { true }";
    let accepted = format!(
        "{prefix} machine run() -> bool {{ match true {{ true -> true, _ -> need(false) }} }}"
    );
    assert!(diagnostics(&accepted).is_empty());
    for expression in ["need(7)", "need()", "need(missing)"] {
        let source = format!(
            "{prefix} machine run() -> bool {{ match true {{ true -> true, _ -> {expression} }} }}"
        );
        assert!(!diagnostics(&source).is_empty(), "{source}");
    }
}

#[test]
fn fresh_match_containers_cannot_hide_existing_owned_inputs_or_cleanup() {
    for source in [
        // A fresh container whose field or element moves an existing owned
        // parameter still joins predecessor custody on the selected edge.
        "data Payload { value: u64; } data Box { payload: Payload; } machine choose(flag: bool, payload: Payload) -> Box { match flag { true -> Box { payload: payload }, false -> Box { payload: Payload { value: 0 } } } }",
        "data Payload { value: u64; } machine choose(flag: bool, payload: Payload) -> [Payload; 1] { match flag { true -> [payload], false -> [Payload { value: 0 }] } }",
        // Wrapping a nominal cleanup hook in a fresh container does not erase
        // the whole-value entitlement its death edge owes.
        "data Payload { value: u64; } machine Payload::drop(&mut self) {} data Box { payload: Payload; } machine choose(flag: bool) -> Box { match flag { true -> Box { payload: Payload { value: 1 } }, false -> Box { payload: Payload { value: 2 } } } }",
        // A linear claim hidden under a fresh affine container is not plain
        // owned storage, whether the child moves an existing input or is
        // itself freshly constructed on each edge.
        "data Payload [linear] { value: u64; } data Box { payload: Payload; } machine choose(flag: bool, payload: Payload) -> Box { match flag { true -> Box { payload: payload }, false -> Box { payload: Payload { value: 0 } } } }",
        "data Payload [linear] { value: u64; } data Box { payload: Payload; } machine choose(flag: bool) -> Box { match flag { true -> Box { payload: Payload { value: 1 } }, false -> Box { payload: Payload { value: 0 } } } }",
        // A fresh linear result whose type also declares nominal cleanup still
        // owes that hook's join; `[linear]` admission does not cover it.
        "data Payload [linear] { value: u64; } machine Payload::drop(&mut self) {} machine choose(flag: bool) -> Payload { match flag { true -> Payload { value: 1 }, false -> Payload { value: 2 } } }",
        // Array literals carry no fresh-linear admission lane, so a linear
        // element joins predecessor custody whether fresh or moved.
        "data Payload [linear] { value: u64; } machine choose(flag: bool) -> [Payload; 1] { match flag { true -> [Payload { value: 1 }], false -> [Payload { value: 2 }] } }",
        "data Payload [linear] { value: u64; } machine choose(flag: bool, payload: Payload) -> [Payload; 1] { match flag { true -> [payload], false -> [Payload { value: 0 }] } }",
    ] {
        let errors = diagnostics(source);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("branch custody join")),
            "{source}: {errors:?}"
        );
    }
    // Deliberate admissions: each edge establishes its own fresh obligation,
    // so the reconvergence agrees on the live ownership frontier and hides no
    // predecessor custody. `ac5ed74dcb` admitted the fresh `[linear]` product
    // (fresh per-edge products join vacuously) and `1016f39c26` admitted a
    // call's structural product as fresh arm storage rather than predecessor
    // custody. Type admission alone: the checker's multiplicity pass still
    // rejects a nonuniform frontier such as one arm moving a live source.
    for admitted in [
        "data Payload [linear] { value: u64; } machine choose(flag: bool) -> Payload { match flag { true -> Payload { value: 1 }, false -> Payload { value: 2 } } }",
        "data Payload { value: u64; } machine fresh() -> Payload { Payload { value: 1 } } machine choose(flag: bool) -> Payload { match flag { true -> fresh(), false -> Payload { value: 2 } } }",
    ] {
        let errors = diagnostics(admitted);
        assert!(errors.is_empty(), "{admitted}: {errors:?}");
    }
}

#[test]
fn match_custody_limits_are_explicit() {
    for (source, expected) in [
        (
            "machine run(flag: bool, value: &mut i64) -> &mut i64 { match flag { true -> value, _ -> value } }",
            "branch custody join",
        ),
        (
            "data Payload { value: i64; } machine Payload::drop(&mut self) {} machine run(flag: bool) -> Payload { match flag { true -> Payload { value: 1 }, _ -> Payload { value: 2 } } }",
            "branch custody join",
        ),
        (
            "data Payload { value: i64; } machine consume(payload: Payload) -> bool { true } machine run(flag: bool, payload: Payload) -> bool { match flag { true -> consume(payload), _ -> false } }",
            "branch-local transfer",
        ),
    ] {
        let errors = diagnostics(source);
        assert!(
            errors.iter().any(|error| error.contains(expected)),
            "{source}: {errors:?}"
        );
    }
    // An immutable owned parameter is already an admitted selection source:
    // the match transfers the chosen ingress value into the result.
    let selected_parameter = "data Payload { value: i64; } machine run(flag: bool, value: Payload) -> Payload { match flag { true -> value, _ -> value } }";
    assert!(
        diagnostics(selected_parameter).is_empty(),
        "{selected_parameter}"
    );
}

#[test]
fn match_result_destinations_do_not_repair_invalid_numeric_arms() {
    for policy in ["", " in Wrapping", " in Saturating", " in Trapping"] {
        let source = format!(
            "machine run(flag: bool) -> u8{policy} {{ match flag {{ _ -> 1, false -> 256 }} }}"
        );
        let errors = diagnostics(&source);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("does not fit destination `u8`")),
            "{source}: {errors:?}"
        );
    }
    for value in [
        "7 / 2",
        "18446744073709551616",
        "18446744073709551616 / 0",
        "18446744073709551616 % 2",
        "1i32",
    ] {
        for pattern in ["true", "_"] {
            let source = format!(
                "machine run(flag: bool) -> u64 {{ match flag {{ {pattern} -> 1, _ -> {value} }} }}"
            );
            assert!(!diagnostics(&source).is_empty(), "{source}");
        }
    }
}
