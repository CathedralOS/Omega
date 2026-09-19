use super::check;

#[test]
fn nested_result_fields_publish_the_selected_count_computation() {
    for (returned, accepted) in [("capacity - length", true), ("capacity", false)] {
        check(
            &format!(
                "data Count [copy] {{ remaining: u64; }}
                 data Issued [copy] {{ count: Count; marker: u64; }}
                 machine allocate(capacity: u64, length: u64) -> Issued
                 requires length <= capacity
                 ensures result.count.remaining == capacity - length
                 {{ Issued {{ count: Count {{ remaining: {returned} }}, marker: 7 }} }}"
            ),
            accepted,
        );
    }
}

#[test]
fn nested_result_fields_use_live_values_not_record_initializers() {
    for (body, accepted) in [
        ("Count { remaining: 7 }", true),
        ("let value: Count = Count { remaining: 7 }; value", true),
        (
            "let mut value: Count = Count { remaining: 7 }; value.remaining = 8; value",
            false,
        ),
    ] {
        check(
            &format!(
                "data Count [copy] {{ remaining: u64; }}
                 machine produce() -> Count ensures result.remaining == 7 {{ {body} }}"
            ),
            accepted,
        );
    }
}

#[test]
fn nested_result_guarantees_do_not_replay_a_captured_source_after_a_write() {
    check(
        "data Count [copy] { remaining: u64; }
         machine replace(value: &mut u64) { value = 8; }
         machine produce(input: &mut Count) -> Count
         ensures result.remaining == input.remaining
         {
             let saved: u64 = input.remaining;
             replace(&mut input.remaining);
             Count { remaining: saved }
         }",
        false,
    );
}

#[test]
fn nested_result_fields_do_not_confuse_arithmetic_policies() {
    check(
        "data Count [copy] { remaining: u8; }
         machine produce(input: u8) -> Count
         ensures result.remaining == ((input as u8 in Saturating) + 1) as u8
         { Count { remaining: ((input as u8 in Wrapping) + 1) as u8 } }",
        false,
    );
}

#[test]
fn result_field_relations_read_current_reference_contents_after_writes() {
    for field in ["spare", "remaining"] {
        check(
            &format!(
                "data Count [copy] {{ remaining: u64; spare: u64; }}
                 machine replace(value: &mut u64) {{ value = 8; }}
                 machine produce(input: &mut Count) -> Count
                 ensures result.remaining == input.remaining
                 {{ replace(&mut input.{field});
                    Count {{ remaining: input.remaining, spare: 7 }} }}"
            ),
            true,
        );
    }
}

#[test]
fn result_field_relations_reject_later_sibling_writes() {
    check(
        "data Count [copy] { remaining: u64; spare: u64; }
         machine replace(value: &mut u64) -> u64 { value = 8; 0 }
         machine produce(input: &mut Count) -> Count
         ensures result.remaining == input.remaining
         { Count { remaining: input.remaining, spare: replace(&mut input.remaining) } }",
        false,
    );
}

#[test]
fn result_field_relations_follow_exact_renamed_scalar_arrivals() {
    for (forwarded, accepted) in [("capacity", true), ("other", false)] {
        check(
            &format!(
                "data Count [copy] {{ remaining: u64; }}
                 machine produce(capacity: u64, other: u64) -> Count
                 ensures result.remaining == capacity
                 {{ transition {{ _ -> finish({forwarded}) }}
                    state finish(current: u64) -> Count {{ Count {{ remaining: current }} }} }}"
            ),
            accepted,
        );
    }
}

#[test]
fn result_field_equality_does_not_assume_float_reflexivity() {
    check(
        "data Reading [copy] { value: f64; }
         machine produce(input: f64) -> Reading ensures result.value == input
         { Reading { value: input } }",
        false,
    );
}

#[test]
fn result_field_arithmetic_does_not_reinterpret_an_authored_operator() {
    check(
        "boundary operator + u64::custom(left: u64, right: u64) -> u64;
         data Count [copy] { remaining: u64; }
         machine produce() -> Count ensures result.remaining == 7
         { Count { remaining: 3u64 + 4u64 } }",
        false,
    );
}

#[test]
fn constant_result_fields_do_not_bypass_authored_equality() {
    check(
        "boundary operator == u64::custom(left: u64, right: u64) -> bool;
         data Count [copy] { remaining: u64; }
         machine produce() -> Count ensures result.remaining == 7u64
         { Count { remaining: 7 } }",
        false,
    );
}

#[test]
fn result_field_relations_do_not_recover_record_origins_from_entry_backedges() {
    check(
        "data Count [copy] { remaining: u64; }
         machine produce(input: Count, again: bool) -> Count
         ensures result.remaining == input.remaining
         { transition again {
             true -> produce(Count { remaining: 8 }, false)
             false -> Count { remaining: input.remaining }
         } }",
        false,
    );
}
