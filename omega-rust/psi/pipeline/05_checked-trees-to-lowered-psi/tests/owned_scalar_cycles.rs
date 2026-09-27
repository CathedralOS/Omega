#[path = "owned_scalar_cycles/execution.rs"]
mod execution;
#[path = "owned_scalar_cycles/mutations.rs"]
mod mutations;
#[path = "owned_scalar_cycles/support.rs"]
mod support;

// Exact rankflow customer, including its ordinary affine data declaration.
const CUSTOMER: &str = r#"machine reset(value: &mut u64) -> u64 { value = 0; 0 }

data Limits {
    limit: u64;
    divisor: u64 [3..=5];
}

machine walk(remaining: u64 [0..=5], limits: Limits, marker: u64)
terminates by remaining -> Nat::Descending in 0..(limits.limit % limits.divisor + 6);
-> u64 {
    let mut scratch: u64 = 0;
    transition remaining > 0 {
        true -> walk(remaining - 1, limits, reset(&mut scratch))
        false -> remaining
    }
}
"#;

fn field_read_customer() -> String {
    CUSTOMER
        .replace(
            "    let mut scratch",
            "    let observed: u64 = limits.limit;\n    let mut scratch",
        )
        .replace("false -> remaining", "false -> observed")
}
