//! Owned inputs retain authored identity through scalar graph publication.

#[path = "owned_scalar_graphs/integer_inputs.rs"]
mod integer_inputs;
#[path = "owned_scalar_graphs/permissions.rs"]
mod permissions;
#[path = "owned_scalar_graphs/record_locals.rs"]
mod record_locals;
#[path = "owned_scalar_graphs/record_moves.rs"]
mod record_moves;
#[path = "owned_scalar_graphs/record_stores.rs"]
mod record_stores;
#[path = "owned_scalar_graphs/source_custody.rs"]
mod source_custody;
#[path = "owned_scalar_graphs/support.rs"]
mod support;
#[path = "owned_scalar_graphs/type_custody.rs"]
mod type_custody;

const LIMITS: &str = r#"
data Limits { limit: u64; divisor: u64 [3..=5]; }
machine reset(value: &mut u64) -> u64 { value = 0; 0 }
machine inspect(marker: u64, limits: Limits) -> u64 {
    let mut scratch: u64 = marker;
    let reset_result: u64 = reset(&mut scratch);
    limits.limit
}
machine enter(limits: Limits, marker: u64) -> u64 {
    let inspected: u64 = inspect(marker, limits);
    inspected
}
"#;

// Nested operands intentionally observe an input after an earlier owned actual.
// Explicit copy semantics keep this order legal; Limits remains ordinary affine data.
const ORDERED: &str = r#"
data Flags [copy] { value: bool; spare: bool; }
machine stamp(value: &mut bool, number: bool) -> bool { value = number; number }
machine choose(before: bool, left: Flags, after: bool, right: Flags) -> bool {
    let selected: bool = before;
    selected
}
machine inspect(marker: bool, left: Flags, other: bool, right: Flags) -> bool {
    let mut scratch: bool = marker;
    let answer: bool = choose(stamp(&mut scratch, left.value), right,
                              stamp(&mut scratch, right.value), left);
    let snapshot: bool = scratch;
    scratch = answer;
    snapshot
}
machine enter(left: Flags, marker: bool, right: Flags, other: bool) -> bool {
    let inspected: bool = inspect(marker, right, other, left);
    inspected
}
"#;
