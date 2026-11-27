//! Evaluate the integer that an atomic read-modify-write installs. Both
//! execution and private-wait admission use the resident's declared width.
use crate::entry_values::TerminalScalarValue;
use terminal_psi::AtomicReadModifyWrite;

pub fn atomic_read_modify_write(
    operation: AtomicReadModifyWrite,
    prior: TerminalScalarValue,
    operand: TerminalScalarValue,
) -> Option<TerminalScalarValue> {
    let (
        TerminalScalarValue::Integer {
            scalar_type,
            value: prior,
        },
        TerminalScalarValue::Integer {
            scalar_type: operand_type,
            value: operand,
        },
    ) = (prior, operand)
    else {
        return None;
    };
    if scalar_type != operand_type || scalar_type.is_address() {
        return None;
    }
    let value = match operation {
        AtomicReadModifyWrite::FetchAdd => scalar_type.wrapping_add(prior, operand),
        AtomicReadModifyWrite::FetchSub => scalar_type.wrapping_sub(prior, operand),
        AtomicReadModifyWrite::FetchAnd => scalar_type.bitwise_and(prior, operand),
        AtomicReadModifyWrite::FetchOr => scalar_type.bitwise_or(prior, operand),
        AtomicReadModifyWrite::FetchXor => scalar_type.bitwise_xor(prior, operand),
    }?;
    Some(TerminalScalarValue::Integer { scalar_type, value })
}
