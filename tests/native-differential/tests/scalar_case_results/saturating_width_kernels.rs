//! Every fixed saturating carrier beyond i32 clamps to its bounds through
//! native publication: signed i8, i16, i64 and unsigned u8, u16, u32, u64
//! for add, subtract, and divide, including both clamp edges, an in-range
//! control, and the signed `MIN / -1` quotient.

use super::{NativeTarget, membership, produce_source, publish};

/// One carrier: its Omega and C spellings and the C bound macros.
struct Carrier {
    omega: &'static str,
    c: &'static str,
    minimum: &'static str,
    maximum: &'static str,
    signed: bool,
}

const CARRIERS: [Carrier; 7] = [
    Carrier {
        omega: "i8",
        c: "int8_t",
        minimum: "INT8_MIN",
        maximum: "INT8_MAX",
        signed: true,
    },
    Carrier {
        omega: "i16",
        c: "int16_t",
        minimum: "INT16_MIN",
        maximum: "INT16_MAX",
        signed: true,
    },
    Carrier {
        omega: "i64",
        c: "int64_t",
        minimum: "INT64_MIN",
        maximum: "INT64_MAX",
        signed: true,
    },
    Carrier {
        omega: "u8",
        c: "uint8_t",
        minimum: "0",
        maximum: "UINT8_MAX",
        signed: false,
    },
    Carrier {
        omega: "u16",
        c: "uint16_t",
        minimum: "0",
        maximum: "UINT16_MAX",
        signed: false,
    },
    Carrier {
        omega: "u32",
        c: "uint32_t",
        minimum: "0",
        maximum: "UINT32_MAX",
        signed: false,
    },
    Carrier {
        omega: "u64",
        c: "uint64_t",
        minimum: "0",
        maximum: "UINT64_MAX",
        signed: false,
    },
];

fn all_targets() -> [NativeTarget; 4] {
    [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ]
}

fn kernel(carrier: &Carrier, operator: &str, divisor_range: &str) -> String {
    let omega = carrier.omega;
    format!(
        "machine kernel(left: {omega}, right: {omega}{divisor_range}) -> {omega} {{
    ((left as {omega} in Saturating) {operator} (right as {omega} in Saturating)) as {omega}
}}"
    )
}

/// The C driver clamps a 128-bit reference to the carrier, so every pair
/// (including `MAX + 1`, `MIN - 1`, `0 - 1`, and `MIN / -1`) is checked
/// against the arithmetic definition rather than against the target.
fn driver(carrier: &Carrier, operator: &str, lefts: &str, rights: &str) -> String {
    let (c, minimum, maximum) = (carrier.c, carrier.minimum, carrier.maximum);
    format!(
        "#include <stdint.h>\n
        extern {c} omega_entry({c}, {c});
        static {c} clamp(__int128 value) {{
            if (value > (__int128){maximum}) return {maximum};
            if (value < (__int128){minimum}) return {minimum};
            return ({c})value;
        }}
        int main(void) {{
            const {c} lefts[] = {{ {lefts} }};
            const {c} rights[] = {{ {rights} }};
            for (unsigned left = 0; left < sizeof lefts / sizeof lefts[0]; ++left)
                for (unsigned right = 0; right < sizeof rights / sizeof rights[0]; ++right) {{
                    {c} expected = clamp((__int128)lefts[left] {operator} (__int128)rights[right]);
                    if (omega_entry(lefts[left], rights[right]) != expected) return 1;
                }}
            return 0;
        }}"
    )
}

fn values(carrier: &Carrier) -> String {
    let (minimum, maximum) = (carrier.minimum, carrier.maximum);
    if carrier.signed {
        format!("{minimum}, {minimum} + 1, -40, -7, -1, 0, 1, 7, 40, {maximum} - 1, {maximum}")
    } else {
        format!("0, 1, 7, 40, {maximum} - 1, {maximum}")
    }
}

fn publish_and_execute(carrier: &Carrier, source: &str, operator: &str, rights: &str) {
    let artifact = produce_source("kernel", source);
    for target in all_targets() {
        publish(&artifact, target);
    }
    membership::execute(
        &artifact,
        &driver(carrier, operator, &values(carrier), rights),
    );
}

#[test]
fn saturating_add_every_width_publishes_four_targets_and_clamps_carrier_edges() {
    for carrier in &CARRIERS {
        publish_and_execute(carrier, &kernel(carrier, "+", ""), "+", &values(carrier));
    }
}

#[test]
fn saturating_subtract_every_width_publishes_four_targets_and_clamps_carrier_edges() {
    for carrier in &CARRIERS {
        publish_and_execute(carrier, &kernel(carrier, "-", ""), "-", &values(carrier));
    }
}

/// Signed carriers take a negative divisor range that includes -1 (so
/// `MIN / -1` is executable and must clamp to MAX) and a positive range;
/// unsigned carriers take one nonzero range and never overflow.
#[test]
fn saturating_divide_every_width_publishes_four_targets_and_clamps_minimum_over_minus_one() {
    for carrier in &CARRIERS {
        let (minimum, maximum) = (carrier.minimum, carrier.maximum);
        let bits = carrier.omega[1..].parse::<u32>().unwrap();
        if carrier.signed {
            let minimum_literal = -(1_i128 << (bits - 1));
            let maximum_literal = (1_i128 << (bits - 1)) - 1;
            publish_and_execute(
                carrier,
                &kernel(carrier, "/", &format!(" [{minimum_literal}..=-1]")),
                "/",
                &format!("{minimum}, {minimum} + 1, -40, -7, -2, -1"),
            );
            publish_and_execute(
                carrier,
                &kernel(carrier, "/", &format!(" [1..={maximum_literal}]")),
                "/",
                &format!("1, 2, 7, 40, {maximum} - 1, {maximum}"),
            );
        } else {
            let maximum_literal = (1_u128 << bits) - 1;
            publish_and_execute(
                carrier,
                &kernel(carrier, "/", &format!(" [1..={maximum_literal}]")),
                "/",
                &format!("1, 2, 7, 40, {maximum} - 1, {maximum}"),
            );
        }
    }
}
