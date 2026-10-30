//! The float-result-policy cases both instruction sets must publish, and the
//! refusals both streams owe. Decoding and executing a stream stays in each
//! instruction set's own test module: a normalised step vocabulary costs more
//! than the second copy it would remove.

use selected_instructions::FloatResultPolicyForm;

/// The smallest subnormal payload, used as an operand and as a raw payload.
const MINIMUM: u64 = 1;

/// The payload a form publishes for one case, or `None` when it must trap.
pub fn expected(form: FloatResultPolicyForm, [left, right, raw]: [u64; 3]) -> Option<u64> {
    let wide = !form.is_binary32();
    if form.can_trap() {
        return number(wide, raw).is_finite().then_some(raw);
    }
    let overflow = number(wide, left).is_finite()
        && number(wide, right).is_finite()
        && number(wide, raw).is_infinite();
    Some(if overflow {
        maximum(wide) | (raw & sign_bit(wide))
    } else {
        raw
    })
}

/// The payloads that carry one case: `raw`, preceded by the selector's
/// eligibility Boolean for a saturating form. Unrelated upper bits are set, so
/// a predicate that reads one of them changes the published payload.
pub fn inputs(form: FloatResultPolicyForm, [left, right, raw]: [u64; 3]) -> Vec<u64> {
    let wide = !form.is_binary32();
    if form.can_trap() {
        return vec![raw];
    }
    let mut inputs = vec![
        u64::from(number(wide, left).is_finite() && number(wide, right).is_finite()),
        raw,
    ];
    // Boolean custody defines its low byte independently of the format.
    inputs[0] |= 0xa5a5_5a5a_dead_be00;
    if !wide {
        for input in &mut inputs {
            *input |= 0xa5a5_5a5a_0000_0000;
        }
    }
    inputs
}

/// Every `[left, right, raw]` case a form must publish for, in the form's own
/// format: signed and unsigned overflow, infinities, zero divisors, NaN
/// payloads on either operand, and every interesting raw payload.
pub fn cases(wide: bool) -> Vec<[u64; 3]> {
    let bits = |number: f64| payload(wide, number);
    let nan_payloads = if wide {
        [0x7ff0_0000_0000_0001, 0xfff8_1234_5678_0000]
    } else {
        [0x7f80_0001, 0xffc1_2345]
    };
    let mut cases = vec![
        [maximum(wide), maximum(wide), bits(f64::INFINITY)],
        [bits(-1.0), maximum(wide), bits(f64::NEG_INFINITY)],
        [bits(f64::INFINITY), bits(1.0), bits(f64::INFINITY)],
        [bits(1.0), bits(f64::NEG_INFINITY), bits(f64::NEG_INFINITY)],
        [bits(1.0), bits(0.0), bits(f64::INFINITY)],
        [bits(1.0), bits(-0.0), bits(f64::NEG_INFINITY)],
        [bits(1.0), MINIMUM, bits(f64::INFINITY)],
        [nan_payloads[0], bits(1.0), bits(f64::INFINITY)],
        [bits(1.0), nan_payloads[1], bits(f64::NEG_INFINITY)],
    ];
    for raw in [
        bits(0.0),
        bits(-0.0),
        MINIMUM,
        maximum(wide),
        bits(1.0),
        nan_payloads[0],
        nan_payloads[1],
    ] {
        cases.push([bits(1.0), bits(2.0), raw]);
    }
    cases
}

/// Require the form to emit exactly `expected` and to validate that stream.
pub fn exact_stream(
    form: FloatResultPolicyForm,
    registers: &[u8],
    expected: &[u8],
    encode: impl Fn(FloatResultPolicyForm, &[u8]) -> Vec<u8>,
    validate: impl Fn(FloatResultPolicyForm, &[u8], &[u8]) -> bool,
) {
    let bytes = encode(form, registers);
    assert_eq!(bytes, expected, "{form:?} must emit its pinned stream");
    assert!(
        validate(form, registers, &bytes),
        "{form:?} must validate its own stream"
    );
}

/// Require every form's stream to refuse each single-bit corruption, a
/// truncated last byte, an appended byte, every other form's claim on the same
/// registers, a swapped output register and a shorter operand list.
pub fn rejects_corrupted_and_substituted_streams(
    roster: impl Fn(FloatResultPolicyForm) -> Vec<u8>,
    encode: impl Fn(FloatResultPolicyForm, &[u8]) -> Vec<u8>,
    validate: impl Fn(FloatResultPolicyForm, &[u8], &[u8]) -> bool,
) {
    for form in FloatResultPolicyForm::ALL {
        let registers = roster(form);
        let bytes = encode(form, &registers);
        assert!(
            validate(form, &registers, &bytes),
            "{form:?} must validate its own stream"
        );
        for position in 0..bytes.len() {
            for bit in 0..8 {
                let mut corrupted = bytes.clone();
                corrupted[position] ^= 1 << bit;
                assert!(
                    !validate(form, &registers, &corrupted),
                    "{form:?} accepted byte {position} bit {bit}"
                );
            }
        }
        assert!(
            !validate(form, &registers, &bytes[..bytes.len() - 1]),
            "{form:?} accepted a truncated last byte"
        );
        let mut appended = bytes.clone();
        appended.push(0x90);
        assert!(
            !validate(form, &registers, &appended),
            "{form:?} accepted an appended byte"
        );
        for other in FloatResultPolicyForm::ALL {
            if other != form {
                assert!(
                    !validate(other, &registers, &bytes),
                    "{other:?} accepted {form:?}'s stream"
                );
            }
        }
        let mut other_registers = registers.clone();
        other_registers.swap(0, registers.len() - 1);
        assert!(
            !validate(form, &other_registers, &bytes),
            "{form:?} accepted a swapped output register"
        );
        assert!(
            !validate(form, &registers[..registers.len() - 1], &bytes),
            "{form:?} accepted a shorter operand list"
        );
    }
}

fn payload(wide: bool, number: f64) -> u64 {
    if wide {
        number.to_bits()
    } else {
        u64::from((number as f32).to_bits())
    }
}

fn number(wide: bool, payload: u64) -> f64 {
    if wide {
        f64::from_bits(payload)
    } else {
        f64::from(f32::from_bits(payload as u32))
    }
}

fn maximum(wide: bool) -> u64 {
    if wide {
        f64::MAX.to_bits()
    } else {
        u64::from(f32::MAX.to_bits())
    }
}

fn sign_bit(wide: bool) -> u64 {
    if wide { 1 << 63 } else { 1 << 31 }
}
