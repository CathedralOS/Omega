//! Pin the ownership boundary: preparation may rewrite typed tables; checking
//! may only read them and publish independently owned facts. Rust's readonly
//! owner prevents mutable field access; this guard also prevents silently
//! restoring preparation inside the checking entrance.

#[test]
fn checking_does_not_reopen_typed_preparation() {
    let source = include_str!("checking.rs");
    for forbidden in [
        "&mut program",
        "&mut typed_program",
        "prepare_typed_trees(",
        "specialize_static_machine_calls(",
        "bind_selected_operator_calls(",
        "resolve_projected_receiver_calls(",
    ] {
        assert!(!source.contains(forbidden), "checking restored {forbidden}");
    }
    assert!(source.contains("program: typed_trees::FrozenTypedTrees"));
}
