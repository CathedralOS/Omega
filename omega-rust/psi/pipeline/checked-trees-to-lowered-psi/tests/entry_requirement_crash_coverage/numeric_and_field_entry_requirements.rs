use super::{
    assert_structural_entry_requirement_artifact, assert_unconditional_call_trap,
    integer_field_entry_source, typed, with_caller,
};
use typed_trees_to_checked_trees::CheckingRequest;
use typed_trees_to_checked_trees::lower_typed_trees;

#[test]
fn numeric_entry_requirement_covers_an_unconditional_call() {
    assert_unconditional_call_trap(&with_caller(
        "machine trigger() -> bool crashes Trap { crash Trap; }\n\
         machine guarded(input: u32) -> bool\n\
         requires input > 0\n\
         crashes Trap input > 0\n\
         { trigger() }",
        "guarded(1)",
    ));
}

#[test]
fn fixed_integer_entry_comparisons_preserve_their_declared_crash_routes() {
    for primitive in ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64"] {
        for (comparison, actual, bound) in [
            (">", 1, 0),
            (">=", 1, 1),
            ("<", 0, 1),
            ("<=", 1, 1),
            ("==", 1, 1),
            ("!=", 1, 0),
        ] {
            assert_unconditional_call_trap(&with_caller(
                &format!(
                    "machine trigger() -> bool crashes Trap {{ crash Trap; }}\n\
                     machine guarded(input: {primitive}) -> bool\n\
                     requires input {comparison} {bound}\n\
                     crashes Trap input {comparison} {bound}\n\
                     {{ trigger() }}",
                ),
                &format!("guarded({actual})"),
            ));
        }
    }
}

#[test]
fn total_comparisons_on_policy_qualified_integers_retain_entry_meaning() {
    for policy in ["", " in Wrapping", " in Saturating", " in Trapping"] {
        assert_unconditional_call_trap(&with_caller(
            &format!(
                "machine trigger() -> bool crashes Trap {{ crash Trap; }}\n\
                 machine guarded(input: u32{policy}) -> bool\n\
                 requires input > 0\n\
                 crashes Trap input > 0\n\
                 {{ trigger() }}",
            ),
            "guarded(1)",
        ));
    }
}

#[test]
fn numeric_entry_requirement_survives_a_body_write() {
    assert_unconditional_call_trap(&with_caller(
        "machine trigger() -> bool crashes Trap { crash Trap; }\n\
         machine guarded(mut input: u32) -> bool\n\
         requires input > 0\n\
         crashes Trap input > 0\n\
         { input = 0; trigger() }",
        "guarded(1)",
    ));
}

#[test]
fn same_carrier_numeric_entry_relations_cover_unconditional_calls() {
    for comparison in ["<", "<=", "!="] {
        let calls: &[&str] = if comparison == "<=" {
            &["guarded(1, 2)", "guarded(1, 1)"]
        } else {
            &["guarded(1, 2)"]
        };
        for call in calls {
            assert_unconditional_call_trap(&with_caller(
                &format!(
                    "machine trigger() -> bool crashes Trap {{ crash Trap; }}\n\
                 machine guarded(left: i32, right: i32) -> bool\n\
                 requires left {comparison} right\n\
                 crashes Trap left {comparison} right\n\
                 {{ trigger() }}",
                ),
                call,
            ));
        }
    }
}

#[test]
fn numeric_entry_formulas_cover_matching_published_formulas() {
    for predicate in [
        "input > 0 && input < 10",
        "input == 1 || input == 2",
        "input < 2 || input > 3",
    ] {
        assert_unconditional_call_trap(&with_caller(
            &format!(
                "machine trigger() -> bool crashes Trap {{ crash Trap; }}\n\
                 machine guarded(input: u32) -> bool\n\
                 requires {predicate}\n\
                 crashes Trap {predicate}\n\
                 {{ trigger() }}",
            ),
            "guarded(1)",
        ));
    }
}

#[test]
fn numeric_entry_bounds_preserve_signed_and_unsigned_carrier_extremes() {
    for (primitive, predicate, actual) in [
        ("i8", "input < -127", "-128"),
        ("i64", "input > 9223372036854775806", "9223372036854775807"),
        (
            "u64",
            "input > 18446744073709551614",
            "18446744073709551615",
        ),
    ] {
        assert_unconditional_call_trap(&with_caller(
            &format!(
                "machine trigger() -> bool crashes Trap {{ crash Trap; }}\n\
                 machine guarded(input: {primitive}) -> bool\n\
                 requires {predicate}\n\
                 crashes Trap {predicate}\n\
                 {{ trigger() }}",
            ),
            &format!("guarded({actual})"),
        ));
    }
}

#[test]
fn numeric_requirements_do_not_authorize_wrong_routes_or_new_body_values() {
    for (requirement, route, body) in [
        ("input > 0", "input == 0", "trigger()"),
        ("input >= 0", "input > 0", "input = 1; trigger()"),
    ] {
        let source = format!(
            "machine trigger() -> bool crashes Trap {{ crash Trap; }}\n\
             machine guarded(mut input: u32) -> bool\n\
             requires {requirement}\n\
             crashes Trap {route}\n\
             {{ {body} }}",
        );
        let diagnostics = lower_typed_trees(typed(&source), &CheckingRequest::settled())
            .expect_err("unproved entry guard");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("uncovered Trap")),
            "{source}: {diagnostics:#?}"
        );
    }
}

#[test]
fn trapping_arithmetic_cannot_become_numeric_entry_crash_evidence() {
    let source = "machine trigger() -> bool crashes Trap { crash Trap; }\n\
                  machine guarded(input: u32 in Trapping) -> bool\n\
                  requires input + 1 > input\n\
                  crashes Trap input > 0\n\
                  { trigger() }";
    let diagnostics = lower_typed_trees(typed(source), &CheckingRequest::settled())
        .expect_err("partial specification term");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("direct Trapping arithmetic")),
        "{diagnostics:#?}"
    );
}

#[test]
fn integer_field_entry_requirement_covers_unconditional_scalar_call() {
    assert_structural_entry_requirement_artifact(
        r#"
        data Record { count: u32; }
        data Helper {}
        data Main {}
        boundary trait Sink { machine record(value: bool); }
        machine trigger() -> bool
        crashes Trap
        { crash Trap; }
        machine Helper::forward(record: &Record)
        reaches Sink requires record.count > 0
        crashes Trap record.count > 0
        { Sink::record(trigger()); }
        machine Main::value(record: &Record)
        requires record.count > 0
        crashes Trap record.count > 0
        { Helper::forward(record); }
        "#,
    );
}

#[test]
fn integer_field_entry_comparisons_retain_all_fixed_carriers_and_relations() {
    for carrier in ["i8", "u8", "i16", "u16", "i32", "u32", "i64", "u64"] {
        for predicate in [
            "record.count == 1",
            "record.count != 0",
            "record.count < 2",
            "record.count <= 2",
            "record.count > 0",
            "record.count >= 1",
            "0 < record.count",
        ] {
            assert_structural_entry_requirement_artifact(&integer_field_entry_source(
                &format!("data Record {{ count: {carrier}; }}"),
                "&Record",
                predicate,
                predicate,
                "",
            ));
        }
    }
}

#[test]
fn integer_field_entry_formulas_preserve_nested_paths_and_root_access() {
    for ownership in ["", "&", "&mut "] {
        for predicate in [
            "record.inner.count > 0",
            "record.inner.count < record.inner.other",
            "record.inner.count <= record.inner.other",
            "record.inner.count != record.inner.other",
            "record.inner.count > 0 && record.inner.count < 10",
            "record.inner.count < 2 || record.inner.count > 3",
            "record.inner.count == 1 || record.inner.count == 2",
            "record.enabled && record.inner.count > 0",
        ] {
            assert_structural_entry_requirement_artifact(&integer_field_entry_source(
                "data Record { #0 count: i32; #11 other: i32; }\n\
                 data Envelope { #7 inner: Record; #9 enabled: bool; }",
                &format!("{ownership}Envelope"),
                predicate,
                predicate,
                "",
            ));
        }
    }
}

#[test]
fn integer_field_comparisons_remain_total_under_every_arithmetic_policy() {
    for policy in ["", " in Wrapping", " in Saturating", " in Trapping"] {
        assert_structural_entry_requirement_artifact(&integer_field_entry_source(
            &format!("data Record {{ count: i32{policy}; }}"),
            "&Record",
            "record.count > 0",
            "record.count > 0",
            "",
        ));
    }
}

#[test]
fn integer_field_entry_requirement_survives_a_body_write() {
    assert_structural_entry_requirement_artifact(&integer_field_entry_source(
        "data Record { count: u32; }",
        "&mut Record",
        "record.count > 0",
        "record.count > 0",
        "record.count = 0;",
    ));
}

#[test]
fn integer_self_field_entry_requirement_covers_an_unconditional_call() {
    assert_structural_entry_requirement_artifact(
        "data Helper { #7 count: i32; }\ndata Main {}\n\
         boundary trait Sink { machine record(value: bool); }\n\
         machine trigger() -> bool\ncrashes Trap\n{ crash Trap; }\n\
         machine Helper::forward(&self)\nrequires self.count > 0\n\
         reaches Sink crashes Trap self.count > 0\n{ Sink::record(trigger()); }\n\
         machine Main::value(record: &Helper)\nrequires record.count > 0\n\
         crashes Trap record.count > 0\n{ record.forward(); }",
    );
}

#[test]
fn integer_field_requirements_do_not_authorize_other_fields_or_body_values() {
    for (requirement, route, body_prefix) in [
        ("record.count > 0", "record.other > 0", ""),
        ("record.count > 0", "record.count == 0", ""),
        ("record.count >= 0", "record.count > 0", "record.count = 1;"),
    ] {
        let source = integer_field_entry_source(
            "data Record { count: i32; other: i32; }",
            "&mut Record",
            requirement,
            route,
            body_prefix,
        );
        let diagnostics = match lower_typed_trees(typed(&source), &CheckingRequest::settled()) {
            Err(diagnostics) => diagnostics,
            Ok(_) => panic!("different entry field/value was accepted: {source}"),
        };
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("Helper::forward")
                    && diagnostic.message.contains("uncovered Trap crash route")
            }),
            "{source}: {diagnostics:#?}"
        );
    }
}

#[test]
fn integer_field_entry_comparisons_preserve_mixed_formal_positions() {
    for (parameters, arguments) in [
        ("limit: u32, record: &Record", "limit, record"),
        ("record: &Record, limit: u32", "record, limit"),
        (
            "other: &Record, limit: u32, record: &Record",
            "other, limit, record",
        ),
    ] {
        let source = format!(
            "data Record {{ count: u32; }}\ndata Helper {{}}\ndata Main {{}}\n\
             boundary trait Sink {{ machine record(value: bool); }}\n\
             machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
             machine Helper::forward({parameters})\nrequires record.count > limit\n\
             reaches Sink crashes Trap record.count > limit\n{{ Sink::record(trigger()); }}\n\
             machine Main::value({parameters})\nrequires record.count > limit\n\
             crashes Trap record.count > limit\n{{ Helper::forward({arguments}); }}"
        );
        assert_structural_entry_requirement_artifact(&source);
    }
}

#[test]
fn integer_field_entry_bounds_retain_carrier_extremes() {
    for (carrier, predicate) in [
        ("i8", "record.count >= -128"),
        ("i64", "record.count <= 9223372036854775807"),
        ("u64", "record.count <= 18446744073709551615"),
    ] {
        assert_structural_entry_requirement_artifact(&integer_field_entry_source(
            &format!("data Record {{ count: {carrier}; }}"),
            "&Record",
            predicate,
            predicate,
            "",
        ));
    }
}
