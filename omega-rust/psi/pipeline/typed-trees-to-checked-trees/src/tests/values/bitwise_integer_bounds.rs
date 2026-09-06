use super::*;

fn accepts(source: &str) {
    lower_typed_trees(typed_trees(source))
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

fn rejects(source: &str, expected: &str) {
    let diagnostics = match lower_typed_trees(typed_trees(source)) {
        Ok(_) => panic!("unsafe integer bitwise bounds were accepted: {source}"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(expected)),
        "expected {expected:?}: {source}\n{diagnostics:#?}"
    );
}

#[test]
fn unsigned_bitwise_results_do_not_become_boolean_overflow_bounds() {
    for (carrier, maximum) in [
        ("u8", "255"),
        ("u16", "65535"),
        ("u32", "4294967295"),
        ("u64", "18446744073709551615"),
    ] {
        for operator in ["&", "|", "^"] {
            let (setup, operand) = if carrier == "u64" {
                (
                    format!("let maximum: u64 = {maximum}u64;"),
                    "maximum".to_owned(),
                )
            } else {
                (String::new(), format!("{maximum}{carrier}"))
            };
            rejects(
                &format!(
                    "machine run(value: {carrier}) -> {carrier} {{
                        {setup}
                        (value {operator} {operand}) + 1{carrier}
                    }}"
                ),
                "may overflow",
            );
        }
    }
}

#[test]
fn signed_bitwise_results_keep_their_integer_width_for_overflow() {
    for (carrier, maximum) in [
        ("i8", "127"),
        ("i16", "32767"),
        ("i32", "2147483647"),
        ("i64", "9223372036854775807"),
    ] {
        for operator in ["&", "|", "^"] {
            rejects(
                &format!(
                    "machine run(value: {carrier}) -> {carrier} {{
                        (value {operator} {maximum}{carrier}) + 1{carrier}
                    }}"
                ),
                "may overflow",
            );
        }
    }
}

#[test]
fn bitwise_integer_returns_do_not_claim_boolean_ranges() {
    for expression in [
        "value | 128u8",
        "value ^ 255u8",
        "value & 255u8",
        "value | 0u8",
        "value ^ 0u8",
    ] {
        rejects(
            &format!("machine run(value: u8) -> u8 [0..=1] {{ {expression} }}"),
            "not provably within its declared range",
        );
    }
}

#[test]
fn low_masks_bound_signed_and_unsigned_bitwise_results() {
    for carrier in ["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64"] {
        accepts(&format!(
            "machine run(value: {carrier}) -> {carrier} [0..=15] {{ value & 15{carrier} }}"
        ));
    }
}

#[test]
fn zero_masks_and_identity_operations_preserve_exact_bounds() {
    for carrier in ["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64"] {
        accepts(&format!(
            "machine run(value: {carrier}) -> {carrier} [0..=0] {{ value & 0{carrier} }}"
        ));
        for operator in ["|", "^"] {
            accepts(&format!(
                "machine run(value: {carrier} [0..=15]) -> {carrier} [0..=15] {{ value {operator} 0{carrier} }}"
            ));
        }
    }
}

#[test]
fn complement_is_width_bounded_not_boolean_negation() {
    accepts("machine run(value: u8 [0..=15]) -> u8 [240..=255] { ~value }");
    accepts("machine run(value: i8 [0..=15]) -> i8 [-16..=-1] { ~value }");
    accepts("machine run() -> u8 [255..=255] { ~0u8 }");
    accepts("machine run() -> i8 [-1..=-1] { ~0i8 }");
    rejects(
        "machine run(value: u8) -> u8 [0..=1] { ~value }",
        "not provably within its declared range",
    );
}

#[test]
fn complement_does_not_hide_following_exact_overflow() {
    for carrier in ["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64"] {
        rejects(
            &format!("machine run(value: {carrier}) -> {carrier} {{ (~value) + 1{carrier} }}"),
            "may overflow",
        );
    }
}

#[test]
fn bitwise_results_preserve_explicit_nonexact_arithmetic_policy() {
    for policy in ["Wrapping", "Saturating"] {
        for operator in ["&", "|", "^"] {
            accepts(&format!(
                "machine run(value: u8 in {policy}) -> u8 in {policy} {{
                    (value {operator} 255u8) + 1u8
                }}"
            ));
        }
        accepts(&format!(
            "machine run(value: u8 in {policy}) -> u8 in {policy} {{ (~value) + 1u8 }}"
        ));
    }
}

#[test]
fn explicit_policy_erasure_restores_exact_overflow_obligations() {
    for operator in ["&", "|", "^"] {
        rejects(
            &format!(
                "machine run(value: u8 in Wrapping) -> u8 {{
                    ((value {operator} 255u8) as u8) + 1u8
                }}"
            ),
            "may overflow",
        );
    }
}

#[test]
fn complement_checks_nested_arithmetic_before_complementing() {
    for carrier in ["u8", "u64", "i8", "i64"] {
        rejects(
            &format!("machine run(value: {carrier}) -> {carrier} {{ ~(value + 1{carrier}) }}"),
            "may overflow",
        );
    }
}

#[test]
fn logical_negation_checks_nested_integer_arithmetic() {
    for carrier in ["u8", "u64", "i8", "i64"] {
        rejects(
            &format!(
                "machine run(value: {carrier}) -> bool {{ !((value + 1{carrier}) == 0{carrier}) }}"
            ),
            "may overflow",
        );
    }
    accepts("machine run(value: u8 [0..=254]) -> bool { !((value + 1u8) == 0u8) }");
}

#[test]
fn u64_complement_and_masks_use_all_sixty_four_bits() {
    accepts(
        "machine run() -> u64 [0..=0] { let maximum: u64 = 18446744073709551615u64; ~maximum }",
    );
    accepts(
        "machine run() -> u64 [9223372036854775807..=9223372036854775807] { let high_bit: u64 = 9223372036854775808u64; ~high_bit }",
    );
    accepts(
        "machine run() -> u64 [0..=0] { let maximum: u64 = 18446744073709551615u64; (~0u64) ^ maximum }",
    );
    accepts(
        "machine run(value: u64 [0..=15]) -> u64 [0..=15] { let maximum: u64 = 18446744073709551615u64; value & maximum }",
    );
    accepts(
        "machine run(value: u64 [0..=15]) -> u64 [0..=0] { let high_bit: u64 = 9223372036854775808u64; value & high_bit }",
    );
    rejects(
        "machine run(value: u64) -> u64 { let high_bit: u64 = 9223372036854775808u64; (value & high_bit) + high_bit }",
        "may overflow",
    );
}

#[test]
fn unknown_u64_ceiling_requires_a_real_overflow_proof() {
    rejects(
        "machine run(value: u64) -> u64 { value + 1u64 }",
        "may overflow",
    );
    accepts("machine run(value: u64) -> u64 { value + 0u64 }");
}

#[test]
fn bitwise_and_interval_does_not_use_only_its_endpoint_results() {
    // Both endpoint results are zero, but every interior value 1..=7 survives
    // this mask. Endpoint-only evaluation cannot establish the result range.
    rejects(
        "machine run(value: u8 [0..=8]) -> u8 [0..=0] { value & 7u8 }",
        "not provably within its declared range",
    );
    accepts("machine run(value: u8 [0..=8]) -> u8 [0..=7] { value & 7u8 }");
}

#[test]
fn literal_suffixes_keep_nested_complement_arithmetic_in_their_carrier() {
    for destination in ["u8", "u16"] {
        rejects(
            &format!("machine run() -> {destination} {{ ~(255u8 + 1u8) }}"),
            "may overflow",
        );
    }
    accepts("machine run() -> u8 [0..=0] { ~(254u8 + 1u8) }");
    accepts("machine run() -> u16 [0..=0] { ~(254u8 + 1u8) }");
    accepts("machine run() -> u16 [255..=255] { ~0u8 }");
}

#[test]
fn logical_negation_cannot_hide_literal_only_integer_overflow() {
    rejects(
        "machine run() -> bool { !((255u8 + 1u8) == 0u8) }",
        "may overflow",
    );
    rejects(
        "machine run() -> bool { !((127i8 + 1i8) == 0i8) }",
        "may overflow",
    );
    accepts("machine run() -> bool { !((254u8 + 1u8) == 0u8) }");
    accepts("machine run() -> bool { !((126i8 + 1i8) == 0i8) }");
}

#[test]
fn wider_destinations_do_not_widen_suffixed_bitwise_arithmetic() {
    for operator in ["&", "|", "^"] {
        let operand = if operator == "&" { "255u8" } else { "0u8" };
        rejects(
            &format!("machine run() -> u16 {{ (255u8 {operator} {operand}) + 1u8 }}"),
            "may overflow",
        );
        accepts(&format!(
            "machine run() -> u16 [255..=255] {{ (254u8 {operator} {operand}) + 1u8 }}"
        ));
    }
}

#[test]
fn overwriting_a_full_width_constant_drops_its_old_bit_pattern() {
    rejects(
        "machine run() -> u64 [0..=0] {
            let mut maximum: u64 = 18446744073709551615u64;
            maximum = 0u64;
            ~maximum
        }",
        "not provably within its declared range",
    );
}

#[test]
fn assignment_can_establish_a_new_full_width_constant() {
    accepts(
        "machine run() -> u64 [0..=0] {
            let mut maximum: u64 = 0u64;
            maximum = 18446744073709551615u64;
            ~maximum
        }",
    );
}

#[test]
fn a_mutating_call_invalidates_a_full_width_constant() {
    rejects(
        "machine clear(value: &mut u64) { value = 0u64; }
        machine run() -> u64 [0..=0] {
            let mut maximum: u64 = 18446744073709551615u64;
            clear(&mut maximum);
            ~maximum
        }",
        "not provably within its declared range",
    );
}

#[test]
fn strict_u64_guard_proves_one_increment_on_either_arm_orientation() {
    for (condition, selected, fallback) in [
        ("index < limit", "index + 1u64", "index"),
        ("index >= limit", "index", "index + 1u64"),
    ] {
        accepts(&format!(
            "machine run(index: u64, limit: u64) -> u64 {{
                transition {condition} {{ true -> ({selected}) false -> ({fallback}) }}
            }}"
        ));
    }
}

#[test]
fn nonstrict_u64_guard_does_not_prove_an_increment() {
    for (condition, selected, fallback) in [
        ("index <= limit", "index + 1u64", "index"),
        ("index > limit", "index", "index + 1u64"),
    ] {
        rejects(
            &format!(
                "machine run(index: u64, limit: u64) -> u64 {{
                    transition {condition} {{ true -> ({selected}) false -> ({fallback}) }}
                }}"
            ),
            "may overflow",
        );
    }
}

#[test]
fn strict_u64_guard_does_not_prove_two_increments() {
    for (condition, selected, fallback) in [
        ("index < limit", "index + 2u64", "index"),
        ("index >= limit", "index", "index + 2u64"),
    ] {
        rejects(
            &format!(
                "machine run(index: u64, limit: u64) -> u64 {{
                    transition {condition} {{ true -> ({selected}) false -> ({fallback}) }}
                }}"
            ),
            "may overflow",
        );
    }
}

#[test]
fn changing_the_guarded_u64_operand_invalidates_its_increment_proof() {
    rejects(
        "machine run(index: u64, limit: u64) -> u64 {
            transition index < limit { true -> increment(index, limit) false -> (index) }
            state increment(mut current: u64, bound: u64) -> u64 {
                current = bound;
                current + 1u64
            }
        }",
        "may overflow",
    );
}

#[test]
fn slice_length_increment_bounds_require_the_strict_selected_arm() {
    for (condition, selected, fallback, accepted) in [
        ("index < items.len", "index + 1u64", "index", true),
        ("index >= items.len", "index", "index + 1u64", true),
        ("index <= items.len", "index + 1u64", "index", false),
    ] {
        let source = format!(
            "machine run(items: &[u8], index: u64) -> u64 {{
                transition {condition} {{ true -> ({selected}) false -> ({fallback}) }}
            }}"
        );
        if accepted {
            accepts(&source);
        } else {
            rejects(&source, "may overflow");
        }
    }
}

#[test]
fn slice_increment_arrivals_retain_the_actual_index_not_a_replacement_length() {
    accepts(
        "machine run(items: &[u8]) -> u64 {
            transition items.len > 0u64 { true -> advance(items, 0u64) false -> 0u64 }
            state advance(values: &[u8], current: u64) -> u64 { current + 1u64 }
        }",
    );
    accepts(
        "machine run(items: &[u8], index: u64) -> u64 {
            transition index < items.len { true -> advance(items, index) false -> (index) }
            state advance(values: &[u8], current: u64) -> u64 { current + 1u64 }
        }",
    );
    rejects(
        "machine run(items: &[u8], index: u64) -> u64 {
            transition index < items.len { true -> advance(items, items.len) false -> (index) }
            state advance(values: &[u8], current: u64) -> u64 { current + 1u64 }
        }",
        "may overflow",
    );
}

#[test]
fn an_authored_len_field_has_only_its_declared_scalar_bounds() {
    accepts(
        "data Metrics { len: u64; }
        machine run(metrics: &Metrics, index: u64) -> u64 {
            transition index < metrics.len { true -> (index + 1u64) false -> (index) }
        }",
    );
    rejects(
        "data Metrics { len: u64; }
        machine run(metrics: &Metrics) -> u64 { metrics.len + 1u64 }",
        "may overflow",
    );
}

#[test]
fn named_boundary_integer_result_preserves_its_declared_carrier() {
    for carrier in ["i32", "u64"] {
        let declaration = format!(
            "data CheckedMath {{}}
            boundary operator CheckedMath::read(value: {carrier}) -> {carrier};"
        );
        accepts(&format!(
            "{declaration}
            machine run(value: {carrier}) -> {carrier} {{ CheckedMath::read(value) + 0{carrier} }}"
        ));
        rejects(
            &format!(
                "{declaration}
                machine run(value: {carrier}) -> {carrier} {{ CheckedMath::read(value) + 1{carrier} }}"
            ),
            "may overflow",
        );
    }
}

#[test]
fn named_boundary_integer_result_mask_proves_its_bounded_return() {
    for carrier in ["i32", "u64"] {
        accepts(&format!(
            "data CheckedMath {{}}
            boundary operator CheckedMath::read(value: {carrier}) -> {carrier};
            machine run(value: {carrier}) -> {carrier} [0..=15] {{
                CheckedMath::read(value) & 15{carrier}
            }}"
        ));
    }
}
