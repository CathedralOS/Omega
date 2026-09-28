use super::{accepts, rejects};

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
