use super::{accepts, rejects};

const IDENTITY: &str =
    "machine accept(delivered: i32) -> i32 ensures result == delivered { delivered }";

#[test]
fn callee_identity_guarantee_uses_the_exact_formal_position() {
    let declarations =
        "machine accept(first: i32, second: i32) -> i32 ensures result == first { first }";
    accepts(&format!(
        "{declarations} machine run() -> i32 ensures result == 6 {{ accept(6, 7 / 2 * 2) }}"
    ));
    rejects(&format!(
        "{declarations} machine run() -> i32 ensures result == 7 {{ accept(6, 7 / 2 * 2) }}"
    ));
}

#[test]
fn unrelated_callee_cannot_borrow_an_identity_guarantee() {
    let declarations =
        format!("{IDENTITY} machine other(delivered: i32) -> i32 ensures result == 0 {{ 0 }}");
    rejects(&format!(
        "{declarations} machine run() -> i32 ensures result == 7 {{ other(7 / 2 * 2) }}"
    ));
}

#[test]
fn copied_call_results_keep_provenance_and_overwrites_retire_it() {
    accepts(&format!(
        "{IDENTITY} machine run() -> i32 ensures result == 7 {{ let saved: i32 = accept(7 / 2 * 2); let copied: i32 = saved; copied }}"
    ));
    let body =
        "let saved: i32 = accept(7 / 2 * 2); let mut current: i32 = saved; current = 8; current";
    accepts(&format!(
        "{IDENTITY} machine run() -> i32 ensures result == 8 {{ {body} }}"
    ));
    rejects(&format!(
        "{IDENTITY} machine run() -> i32 ensures result == 7 {{ {body} }}"
    ));
}

#[test]
fn mutable_formals_do_not_preserve_their_delivered_entry_value() {
    let declarations = "machine change(mut delivered: i32) -> i32 ensures result == delivered { delivered = 8; delivered }";
    accepts(&format!(
        "{declarations} machine run() -> i32 {{ change(7 / 2 * 2) }}"
    ));
    rejects(&format!(
        "{declarations} machine run() -> i32 ensures result == 7 {{ change(7 / 2 * 2) }}"
    ));
}
