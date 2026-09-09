//! Every primitive width observes the borrowed local after replacement.
use super::{produce, publication};

fn source(scalar: &str) -> String {
    format!(
        "machine replace(destination: &mut {scalar}, value: {scalar}) {{
            destination = value;
        }}
        machine observe(output: &mut {scalar}, initial: {scalar}, replacement: {scalar}) {{
            let mut scratch: {scalar} = initial;
            replace(&mut scratch, replacement);
            output = scratch;
        }}"
    )
}

#[test]
fn integer_widths_publish_exact_local_observations() {
    for scalar in ["u8", "i8", "u16", "i16", "u32", "i32", "u64", "i64"] {
        let artifact = produce(&source(scalar), "observe");
        publication::assert_four_targets(&artifact);
        publication::assert_host_execution(&artifact, &driver(scalar, false));
    }
}

#[test]
fn boolean_locals_publish_exact_observations() {
    let artifact = produce(&source("bool"), "observe");
    publication::assert_four_targets(&artifact);
    publication::assert_host_execution(&artifact, &driver("bool", false));
}

#[test]
fn narrow_stack_arguments_preserve_signed_unsigned_and_boolean_bits() {
    for scalar in ["u8", "i8", "u16", "i16", "bool"] {
        let source = source(scalar)
            .replace(
                &format!("destination: &mut {scalar}, value: {scalar}"),
                &format!(
                    "destination: &mut {scalar}, first: u64, second: u64, third: u64, \
                     fourth: u64, fifth: u64, sixth: u64, seventh: u64, eighth: u64, value: {scalar}"
                ),
            )
            .replace(
                "replace(&mut scratch, replacement);",
                "replace(&mut scratch, 1, 2, 3, 4, 5, 6, 7, 8, initial);\n\
                 replace(&mut scratch, 1, 2, 3, 4, 5, 6, 7, 8, replacement);",
            );
        let artifact = produce(&source, "observe");
        publication::assert_four_targets(&artifact);
        publication::assert_host_execution(&artifact, &driver(scalar, false));
    }
}

#[test]
fn ieee_locals_publish_exact_payload_observations() {
    for scalar in ["f32", "f64"] {
        let artifact = produce(&source(scalar), "observe");
        publication::assert_four_targets(&artifact);
        publication::assert_host_execution(&artifact, &driver(scalar, false));
    }
}

#[test]
fn primitive_reads_stop_at_the_referent_end() {
    for scalar in [
        "u8", "i8", "u16", "i16", "u32", "i32", "u64", "i64", "bool", "f32", "f64",
    ] {
        let source = format!(
            "machine replace(destination: &mut {scalar}, value: {scalar}) {{
                destination = value;
            }}
            machine observe(output: &mut {scalar}, input: &{scalar},
                initial_output: &mut {scalar}, replacement: {scalar}) {{
                let mut scratch: {scalar} = input;
                let before: {scalar} = scratch;
                replace(&mut scratch, replacement);
                output = scratch;
                initial_output = before;
            }}"
        );
        let artifact = produce(&source, "observe");
        publication::assert_four_targets(&artifact);
        publication::assert_host_execution(&artifact, &driver(scalar, true));
    }
}

fn driver(scalar: &str, guarded: bool) -> String {
    let (carrier, patterns) = match scalar {
        "bool" => ("_Bool", "0, 1"),
        "u8" => ("uint8_t", "0, 1, 127, 128, 254, 255"),
        "i8" => ("int8_t", "0, 1, 127, 128, 254, 255"),
        "u16" => ("uint16_t", "0, 1, 32767, 32768, 65534, 65535"),
        "i16" => ("int16_t", "0, 1, 32767, 32768, 65534, 65535"),
        "u32" => (
            "uint32_t",
            "0, 1, 0x7fffffff, 0x80000000, 0xfffffffe, 0xffffffff",
        ),
        "i32" => (
            "int32_t",
            "0, 1, 0x7fffffff, 0x80000000, 0xfffffffe, 0xffffffff",
        ),
        "u64" => (
            "uint64_t",
            "0, 1, 0x7fffffffffffffffULL, 0x8000000000000000ULL, 0xfffffffffffffffeULL, 0xffffffffffffffffULL",
        ),
        "i64" => (
            "int64_t",
            "0, 1, 0x7fffffffffffffffULL, 0x8000000000000000ULL, 0xfffffffffffffffeULL, 0xffffffffffffffffULL",
        ),
        "f32" => (
            "float",
            "0, 0x80000000, 1, 0x007fffff, 0x7f800000, 0xff800000, 0x7fc12345, 0x7f812345",
        ),
        "f64" => (
            "double",
            "0, 0x8000000000000000ULL, 1, 0x000fffffffffffffULL, 0x7ff0000000000000ULL, 0xfff0000000000000ULL, 0x7ff8123456789abcULL, 0x7ff0123456789abcULL",
        ),
        _ => panic!("no primitive driver for {scalar}"),
    };
    let template = if guarded {
        include_str!("guarded_reads.c")
    } else {
        include_str!("widths.c")
    };
    template
        .replace("OMEGA_SCALAR", carrier)
        .replace("OMEGA_PATTERNS", patterns)
}
